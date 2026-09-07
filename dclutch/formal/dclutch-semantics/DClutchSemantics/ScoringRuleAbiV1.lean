import DClutchSemantics.AbiSchema
import DClutchSemantics.AbiCoverage
import DClutchSemantics.ScoringRuleV1

/-!
# ScoringRuleAbiV1: the scoring Dealer's records and wires

`ScoringRuleV1` owns the rule -- the potential, the prices, the bounded-loss
theorem and the 112-byte sealed rule record.  This module owns what the chain
persists AROUND that rule and what a caller sends to move it:

* the **fund** -- the Dealer's cash (a `TradingPrincipal` Custody vault, atoms),
  and the two integers from which `Ŵ(inv)` is read back without a logarithm:
  `inventoryMinimum` and `liquidityCost`, so `Ŵ = minimum − cost`;
* the **quote** -- one account per Dealer, rewritten by every `DealerQuote`,
  carrying the price vector `p̂(inv)` at the fund revision it was derived at;
* the four **requests** (found, quote, fill, withdraw) and the one **receipt**
  every route returns;
* the **fill witness** the accelerator's Dealer arm evaluates: the rule, the
  inventory and the fund scalars the request is checked against.

Every width below is a theorem (`decide`), every offset is placed by
`AbiSchema.specialize` and never typed, and `tiles` says the fields cover the
declared width with no unowned byte.  The Rust and TypeScript modules are
emitted from this file (`EmitScoringRuleV1Rust.lean`, `EmitScoringRuleV1Ts.lean`).

## Signed quantities

The AbiSchema has no signed kind, and none is wanted: `Ŵ` is carried as the
pair `(minimum, cost)` of naturals, and a fill's cash movement as the pair
`(dealerPays, dealerReceives)` of which at most one is nonzero.  A reader who
needs the integer subtracts; a writer who has an integer splits it.  This keeps
every persisted number a `Nat` the theorems in `ScoringRuleV1` speak about.

## Provisional rulings this module encodes (BUILD-DEALER, 2026-09-06)

* The scoring Dealer is founded AFTER the Market is Open by its own route
  (`DealerFound`), by a sponsor who may be anyone; it does not ride the generic
  Market founding.  The founding input is `--dealer-fund ATOMS --dealer-b B`.
* One quote account per Dealer, rewritten in place; a quote is FRESH iff its
  `fundRevision` equals the fund's current revision.  `DealerFill` never reads
  the quote: R2 against the post-fill state is the only price authority, so a
  stale quote can mislead a reader and nothing else.
* The fund's refund source on retirement is the sponsor recorded at founding;
  `DealerWithdraw` may take the fund to exactly `Φ = cash + Ŵ` (the
  `withdraw_floor` theorem) and a retired Dealer's residue returns to that
  sponsor through the same route once the Market is terminal.
-/

namespace DClutch.ScoringRuleAbiV1

open DClutch.AbiSchema

/-- Fixed capacity of every per-outcome vector; the Dealer profile's
provisional `maxOutcomes` (`DealerLiquidityAbi.lean`). -/
def maxOutcomes : Nat := 16

/-- Bytes of one fixed-capacity `u64` vector. -/
def vectorBytes : Nat := maxOutcomes * 8

theorem vector_is_128_bytes : vectorBytes = 128 := by decide

/-- Every record and wire this module owns shares one version. -/
def wireVersion : Nat := 1

/-! ## Magics -/

def fundMagic : String := "DCLSFUN1"
def quoteMagic : String := "DCLSQUO1"
def foundRequestMagic : String := "DCLSFDR1"
def quoteRequestMagic : String := "DCLSQTR1"
def fillRequestMagic : String := "DCLSFLR1"
def withdrawRequestMagic : String := "DCLSWDR1"
def receiptMagic : String := "DCLSRCP1"
def fillWitnessMagic : String := "DCLSFLW1"

/-- `ScoringRuleV1.ruleMagic` as the string the other magics are spelled in. -/
def ruleMagic : String := "DCLSCR01"

theorem rule_magic_agrees :
    ruleMagic.toUTF8.toList = DClutch.ScoringRuleV1.ruleMagic := by native_decide

/-! ## PDA seed domains (all under the Trading program) -/

def rulePdaDomain : String := "dclutch:scoring-rule:v1"
def fundPdaDomain : String := "dclutch:dealer-fund:v1"
def quotePdaDomain : String := "dclutch:dealer-quote:v1"

/-- The fund's Custody vault context is the fund address itself, so the
`TradingPrincipal` vault is `CustodyVaultSeedsV1(market, releaseSet, fund,
TradingPrincipal)` and the census derives it from the fund alone. -/
def vaultContextIsTheFund : Bool := true

/-! ## The refusal sub-band -/

/-- Offset of the scoring Dealer's sub-band inside Trading's band
(decision 0007): codes `0x4200 ..`.

`0x100` was taken.  `SeriesAccountErrorV3`
(`programs/dclutch-trading-sbf/src/series/accounts.rs:44-52`) has held
`0x4100 .. 0x4104` since before this family existed, and the build wave chose
`0x100` without asking the census; `tools/gate census` convicted all five
overlaps at once.  Bands are append-only and a code is never renumbered once it
can reach a chain -- no scoring Dealer code ever has, so this family moves and
Series keeps what it already published. -/
def refusalSubBandOffset : Nat := 0x200

/-! ## The fund record -/

inductive FundField where
  | magic | version | outcomeCount | phase | reserved | marketId | dealerId
  | sponsor | ruleDigest | vault | claimUnitAtoms | cash | inventoryMinimum
  | liquidityCost | revision | bump | reservedTail
  deriving DecidableEq, Repr

def fundSchema : List (FieldSpec FundField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩, ⟨.phase, .u8⟩,
  ⟨.reserved, .reserved 4⟩, ⟨.marketId, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩,
  ⟨.sponsor, .bytes 32⟩, ⟨.ruleDigest, .bytes 32⟩, ⟨.vault, .bytes 32⟩,
  ⟨.claimUnitAtoms, .u64⟩, ⟨.cash, .u64⟩, ⟨.inventoryMinimum, .u64⟩,
  ⟨.liquidityCost, .u64⟩, ⟨.revision, .u64⟩, ⟨.bump, .u8⟩, ⟨.reservedTail, .reserved 7⟩
]

def fundLayout : List (PlacedField FundField) := specialize fundSchema
def fundBytes : Nat := schemaWidth fundSchema

theorem fund_is_224_bytes : fundBytes = 224 := by decide
theorem fund_tiles : tiles 0 fundLayout fundBytes = true := by decide

/-- Fund phases. -/
def fundPhaseOpen : UInt8 := 0
def fundPhaseRetired : UInt8 := 1

/-! ## The quote record -/

inductive QuoteField where
  | magic | version | outcomeCount | reserved | marketId | dealerId
  | fundRevision | slot | scale | prices | bump | reservedTail
  deriving DecidableEq, Repr

def quoteSchema : List (FieldSpec QuoteField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.marketId, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩,
  ⟨.fundRevision, .u64⟩, ⟨.slot, .u64⟩, ⟨.scale, .u64⟩,
  ⟨.prices, .nested vectorBytes⟩, ⟨.bump, .u8⟩, ⟨.reservedTail, .reserved 7⟩
]

def quoteLayout : List (PlacedField QuoteField) := specialize quoteSchema
def quoteBytes : Nat := schemaWidth quoteSchema

theorem quote_is_240_bytes : quoteBytes = 240 := by decide
theorem quote_tiles : tiles 0 quoteLayout quoteBytes = true := by decide

/-! ## The four requests -/

inductive FoundRequestField where
  | magic | version | outcomeCount | reserved | market | dealerId | releaseSet
  | liquidity | scale | tolerance | deposit | claimUnitAtoms | generation
  deriving DecidableEq, Repr

def foundRequestSchema : List (FieldSpec FoundRequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.market, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.liquidity, .u64⟩, ⟨.scale, .u64⟩,
  ⟨.tolerance, .u64⟩, ⟨.deposit, .u64⟩, ⟨.claimUnitAtoms, .u64⟩, ⟨.generation, .u64⟩
]

def foundRequestLayout := specialize foundRequestSchema
def foundRequestBytes : Nat := schemaWidth foundRequestSchema

theorem found_request_is_160_bytes : foundRequestBytes = 160 := by decide
theorem found_request_tiles : tiles 0 foundRequestLayout foundRequestBytes = true := by decide

inductive QuoteRequestField where
  | magic | version | reserved | market | dealerId | expectedFundRevision
  deriving DecidableEq, Repr

def quoteRequestSchema : List (FieldSpec QuoteRequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reserved, .reserved 6⟩,
  ⟨.market, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩, ⟨.expectedFundRevision, .u64⟩
]

def quoteRequestLayout := specialize quoteRequestSchema
def quoteRequestBytes : Nat := schemaWidth quoteRequestSchema

theorem quote_request_is_88_bytes : quoteRequestBytes = 88 := by decide
theorem quote_request_tiles : tiles 0 quoteRequestLayout quoteRequestBytes = true := by decide

/-- The fill: the taker's proposed uniform price vector `p̂` (R2 holds it to
`p̂(inv′)` within `τ`), `mint` complete sets minted at par into the fill
(funded by both parties, the complete-set move of the batch of two), the
Dealer's `receive` and `deliver` vectors exactly as `ScoringRuleV1.Fill`.
The taker's Position moves by `mint + deliver − receive` per outcome and the
Dealer's by `receive − deliver`; the aggregate by `mint` everywhere.  The
debit is DERIVED on chain (`roundedQuoteFor`: receipt rounded up, delivery
rounded down, once) and never carried on the wire. -/
inductive FillRequestField where
  | magic | version | outcomeCount | reserved | market | dealerId | taker
  | expectedFundRevision | mint | prices | receive | deliver
  deriving DecidableEq, Repr

def fillRequestSchema : List (FieldSpec FillRequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.market, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩,
  ⟨.taker, .bytes 32⟩, ⟨.expectedFundRevision, .u64⟩, ⟨.mint, .u64⟩,
  ⟨.prices, .nested vectorBytes⟩, ⟨.receive, .nested vectorBytes⟩,
  ⟨.deliver, .nested vectorBytes⟩
]

def fillRequestLayout := specialize fillRequestSchema
def fillRequestBytes : Nat := schemaWidth fillRequestSchema

theorem fill_request_is_512_bytes : fillRequestBytes = 512 := by decide
theorem fill_request_tiles : tiles 0 fillRequestLayout fillRequestBytes = true := by decide

inductive WithdrawRequestField where
  | magic | version | reserved | market | dealerId | expectedFundRevision | amount
  deriving DecidableEq, Repr

def withdrawRequestSchema : List (FieldSpec WithdrawRequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reserved, .reserved 6⟩,
  ⟨.market, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩, ⟨.expectedFundRevision, .u64⟩,
  ⟨.amount, .u64⟩
]

def withdrawRequestLayout := specialize withdrawRequestSchema
def withdrawRequestBytes : Nat := schemaWidth withdrawRequestSchema

theorem withdraw_request_is_96_bytes : withdrawRequestBytes = 96 := by decide
theorem withdraw_request_tiles :
    tiles 0 withdrawRequestLayout withdrawRequestBytes = true := by decide

/-! ## The one receipt -/

/-- Which route produced the receipt. -/
def routeFound : UInt8 := 0
def routeQuote : UInt8 := 1
def routeFill : UInt8 := 2
def routeWithdraw : UInt8 := 3

inductive ReceiptField where
  | magic | version | route | outcomeCount | reserved | requestDigest | market
  | dealerId | fundRevision | cash | inventoryMinimum | liquidityCost
  | dealerPays | dealerReceives | fundDigest
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.route, .u8⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 4⟩, ⟨.requestDigest, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.dealerId, .bytes 32⟩, ⟨.fundRevision, .u64⟩, ⟨.cash, .u64⟩,
  ⟨.inventoryMinimum, .u64⟩, ⟨.liquidityCost, .u64⟩, ⟨.dealerPays, .u64⟩,
  ⟨.dealerReceives, .u64⟩, ⟨.fundDigest, .bytes 32⟩
]

def receiptLayout := specialize receiptSchema
def receiptBytes : Nat := schemaWidth receiptSchema

theorem receipt_is_192_bytes : receiptBytes = 192 := by decide
theorem receipt_tiles : tiles 0 receiptLayout receiptBytes = true := by decide

/-! ## The fill witness the accelerator evaluates -/

inductive FillWitnessField where
  | magic | version | outcomeCount | reserved | rule | inventory | fundRevision | cash
  deriving DecidableEq, Repr

def fillWitnessSchema : List (FieldSpec FillWitnessField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.rule, .nested DClutch.ScoringRuleV1.ruleBytes⟩,
  ⟨.inventory, .nested vectorBytes⟩, ⟨.fundRevision, .u64⟩, ⟨.cash, .u64⟩
]

def fillWitnessLayout := specialize fillWitnessSchema
def fillWitnessBytes : Nat := schemaWidth fillWitnessSchema

theorem fill_witness_is_272_bytes : fillWitnessBytes = 272 := by decide
theorem fill_witness_tiles : tiles 0 fillWitnessLayout fillWitnessBytes = true := by decide

/-! ## Account frames

Each route's frame is stated here so the Rust coordinates and the browser's
lookup-table planner are one list.  `(writable, signer)` per coordinate; every
program account is executable and readonly. -/

structure FrameSlot where
  name : String
  writable : Bool
  signer : Bool
  deriving Repr

def foundFrame : List FrameSlot := [
  ⟨"sponsor", true, true⟩,            -- 0  pays rent, deposits, is recorded as the refund source
  ⟨"fund", true, false⟩,              -- 1  the fund PDA, created here
  ⟨"rule", true, false⟩,              -- 2  the rule PDA, created here (sealed at founding)
  ⟨"quote", true, false⟩,             -- 3  the quote PDA, created here, first quote written
  ⟨"market", false, false⟩,           -- 4  Core Market state, Open
  ⟨"aggregate", true, false⟩,         -- 5  Claims aggregate (the Position admission reads it)
  ⟨"dealer_position", true, false⟩,   -- 6  the Dealer's Claims Position PDA (owner = fund)
  ⟨"dealer_admission", true, false⟩,  -- 7  the Claims admission record for that Position
  ⟨"claims_authority", false, false⟩, -- 8  Trading's caller-authority PDA toward Claims
  ⟨"claims_program", false, false⟩,   -- 9
  ⟨"sponsor_token", true, false⟩,     -- 10 the sponsor's collateral token account
  ⟨"vault", true, false⟩,             -- 11 the fund's TradingPrincipal vault, opened here
  ⟨"custody_replay", true, false⟩,    -- 12 the Custody replay cursor for the fund context
  ⟨"custody_authority", false, false⟩,-- 13 Trading's caller-authority PDA toward Custody
  ⟨"custody_program", false, false⟩,  -- 14
  ⟨"mint", false, false⟩,             -- 15
  ⟨"token_program", false, false⟩,    -- 16
  ⟨"activation_cache", false, false⟩, -- 17 Registry activation cache for the release set
  ⟨"registry_program", false, false⟩, -- 18
  ⟨"system_program", false, false⟩,   -- 19
  ⟨"rent", false, false⟩              -- 20
]

def quoteFrame : List FrameSlot := [
  ⟨"payer", true, true⟩,              -- 0  pays nothing but the fee; permissionless
  ⟨"fund", false, false⟩,             -- 1
  ⟨"rule", false, false⟩,             -- 2
  ⟨"quote", true, false⟩,             -- 3  rewritten in place
  ⟨"market", false, false⟩,           -- 4
  ⟨"dealer_position", false, false⟩,  -- 5  the inventory the quote is derived from
  ⟨"activation_cache", false, false⟩, -- 6  which program owns a Position
  ⟨"registry_program", false, false⟩  -- 7  the cache's own owner
]

def fillFrame : List FrameSlot := [
  ⟨"taker", true, true⟩,              -- 0
  ⟨"fund", true, false⟩,              -- 1
  ⟨"rule", false, false⟩,             -- 2
  ⟨"market", false, false⟩,           -- 3
  ⟨"aggregate", true, false⟩,         -- 4  Claims aggregate (supply moves by `mint`)
  ⟨"dealer_position", true, false⟩,   -- 5
  ⟨"taker_position", true, false⟩,    -- 6
  ⟨"claims_authority", false, false⟩, -- 7
  ⟨"claims_program", false, false⟩,   -- 8
  ⟨"taker_token", true, false⟩,       -- 9
  ⟨"vault", true, false⟩,             -- 10 the fund's TradingPrincipal vault
  ⟨"hoard", true, false⟩,             -- 11 the Market's HoardPrincipal vault (par of the mint)
  ⟨"custody_replay", true, false⟩,    -- 12
  ⟨"custody_authority", false, false⟩,-- 13
  ⟨"custody_program", false, false⟩,  -- 14
  ⟨"mint", false, false⟩,             -- 15
  ⟨"token_program", false, false⟩,    -- 16
  ⟨"activation_cache", false, false⟩, -- 17
  ⟨"registry_program", false, false⟩  -- 18
]

def withdrawFrame : List FrameSlot := [
  ⟨"sponsor", true, true⟩,            -- 0
  ⟨"fund", true, false⟩,              -- 1
  ⟨"rule", false, false⟩,             -- 2
  ⟨"market", false, false⟩,           -- 3
  ⟨"dealer_position", false, false⟩,  -- 4  the inventory Φ is read at
  ⟨"sponsor_token", true, false⟩,     -- 5
  ⟨"vault", true, false⟩,             -- 6
  ⟨"custody_replay", true, false⟩,    -- 7
  ⟨"custody_authority", false, false⟩,-- 8
  ⟨"custody_program", false, false⟩,  -- 9
  ⟨"mint", false, false⟩,             -- 10
  ⟨"token_program", false, false⟩,    -- 11
  ⟨"activation_cache", false, false⟩, -- 12
  ⟨"registry_program", false, false⟩  -- 13
]

theorem found_frame_is_21 : foundFrame.length = 21 := by decide
/-- The quote frame carries the release waist because the price it writes is a
CHAIN fact and not a projection: a Position is only a Position if the program
that owns it is the Claims role this release set activated, and nothing else in
a six-account frame could say so.  Without it the route derived the Position's
address under the account's OWN `owner`, so any program could mint an account
that satisfied the derivation and quote whatever price it liked. -/
theorem quote_frame_is_8 : quoteFrame.length = 8 := by decide
theorem fill_frame_is_19 : fillFrame.length = 19 := by decide
theorem withdraw_frame_is_14 : withdrawFrame.length = 14 := by decide

/-- Exactly one signer per frame, at coordinate zero. -/
def oneSignerAtZero (frame : List FrameSlot) : Bool :=
  match frame with
  | [] => false
  | head :: rest => head.signer && rest.all (fun slot => !slot.signer)

theorem frames_have_one_signer :
    oneSignerAtZero foundFrame && oneSignerAtZero quoteFrame &&
      oneSignerAtZero fillFrame && oneSignerAtZero withdrawFrame = true := by decide

/-! ## The fill's cash legs, as the route derives them

`mintAtoms = mint · claimUnitAtoms` must reach the Hoard; the Dealer's net
cash out is `debitAtoms = (dealerPays − dealerReceives) · claimUnitAtoms`;
the taker pays the rest.  Two Custody transfers at most.  Stated here so the
census (L8) and the route agree on which compartments move by how much. -/

structure CashLegs where
  takerToHoard : Nat
  fundToHoard : Nat
  fundToTaker : Nat
  takerToFund : Nat
  deriving Repr, DecidableEq

def cashLegs (mintAtoms dealerPaysAtoms dealerReceivesAtoms : Nat) : CashLegs :=
  if dealerReceivesAtoms = 0 then
    -- the Dealer pays `d`; it pays into the Hoard first, then the taker
    let d := dealerPaysAtoms
    let fundToHoard := min d mintAtoms
    { takerToHoard := mintAtoms - fundToHoard,
      fundToHoard := fundToHoard,
      fundToTaker := d - fundToHoard,
      takerToFund := 0 }
  else
    -- the Dealer receives `r`; the taker funds the whole mint and pays `r` more
    { takerToHoard := mintAtoms, fundToHoard := 0, fundToTaker := 0,
      takerToFund := dealerReceivesAtoms }

/-- The Hoard receives exactly the mint's par, whichever way the legs fall. -/
theorem hoard_receives_the_mint (m p : Nat) :
    (cashLegs m p 0).takerToHoard + (cashLegs m p 0).fundToHoard = m := by
  simp only [cashLegs, if_true]
  omega

theorem hoard_receives_the_mint' (m r : Nat) (h : r ≠ 0) :
    (cashLegs m 0 r).takerToHoard + (cashLegs m 0 r).fundToHoard = m := by
  simp only [cashLegs, h, if_false]
  omega

/-- The fund's net movement is exactly the Dealer's debit. -/
theorem fund_moves_by_the_debit (m p : Nat) :
    (cashLegs m p 0).fundToHoard + (cashLegs m p 0).fundToTaker = p := by
  simp only [cashLegs, if_true]
  omega

/-! ## Names the two emitters print

One naming function per record, here rather than in the emitters, so the Rust
and TypeScript constants are the same identifier for the same field. -/

def fieldOffset [DecidableEq α] (layout : List (PlacedField α)) (name : α) : Nat :=
  match layout.find? (fun field => field.spec.name = name) with
  | some field => field.offset
  | none => 0

def RuleFieldName : DClutch.ScoringRuleV1.RuleField → String
  | .magic => "RULE_MAGIC_OFFSET"
  | .version => "RULE_VERSION_OFFSET"
  | .outcomeCount => "RULE_OUTCOME_COUNT_OFFSET"
  | .reserved => "RULE_RESERVED_OFFSET"
  | .marketId => "RULE_MARKET_ID_OFFSET"
  | .dealerId => "RULE_DEALER_ID_OFFSET"
  | .liquidity => "RULE_LIQUIDITY_OFFSET"
  | .scale => "RULE_SCALE_OFFSET"
  | .tolerance => "RULE_TOLERANCE_OFFSET"
  | .subsidy => "RULE_SUBSIDY_OFFSET"

def FundField.constantName : FundField → String
  | .magic => "FUND_MAGIC_OFFSET"
  | .version => "FUND_VERSION_OFFSET"
  | .outcomeCount => "FUND_OUTCOME_COUNT_OFFSET"
  | .phase => "FUND_PHASE_OFFSET"
  | .reserved => "FUND_RESERVED_OFFSET"
  | .marketId => "FUND_MARKET_ID_OFFSET"
  | .dealerId => "FUND_DEALER_ID_OFFSET"
  | .sponsor => "FUND_SPONSOR_OFFSET"
  | .ruleDigest => "FUND_RULE_DIGEST_OFFSET"
  | .vault => "FUND_VAULT_OFFSET"
  | .claimUnitAtoms => "FUND_CLAIM_UNIT_ATOMS_OFFSET"
  | .cash => "FUND_CASH_OFFSET"
  | .inventoryMinimum => "FUND_INVENTORY_MINIMUM_OFFSET"
  | .liquidityCost => "FUND_LIQUIDITY_COST_OFFSET"
  | .revision => "FUND_REVISION_OFFSET"
  | .bump => "FUND_BUMP_OFFSET"
  | .reservedTail => "FUND_RESERVED_TAIL_OFFSET"

def QuoteField.constantName : QuoteField → String
  | .magic => "QUOTE_MAGIC_OFFSET"
  | .version => "QUOTE_VERSION_OFFSET"
  | .outcomeCount => "QUOTE_OUTCOME_COUNT_OFFSET"
  | .reserved => "QUOTE_RESERVED_OFFSET"
  | .marketId => "QUOTE_MARKET_ID_OFFSET"
  | .dealerId => "QUOTE_DEALER_ID_OFFSET"
  | .fundRevision => "QUOTE_FUND_REVISION_OFFSET"
  | .slot => "QUOTE_SLOT_OFFSET"
  | .scale => "QUOTE_SCALE_OFFSET"
  | .prices => "QUOTE_PRICES_OFFSET"
  | .bump => "QUOTE_BUMP_OFFSET"
  | .reservedTail => "QUOTE_RESERVED_TAIL_OFFSET"

def FoundRequestField.constantName : FoundRequestField → String
  | .magic => "FOUND_REQUEST_MAGIC_OFFSET"
  | .version => "FOUND_REQUEST_VERSION_OFFSET"
  | .outcomeCount => "FOUND_REQUEST_OUTCOME_COUNT_OFFSET"
  | .reserved => "FOUND_REQUEST_RESERVED_OFFSET"
  | .market => "FOUND_REQUEST_MARKET_OFFSET"
  | .dealerId => "FOUND_REQUEST_DEALER_ID_OFFSET"
  | .releaseSet => "FOUND_REQUEST_RELEASE_SET_OFFSET"
  | .liquidity => "FOUND_REQUEST_LIQUIDITY_OFFSET"
  | .scale => "FOUND_REQUEST_SCALE_OFFSET"
  | .tolerance => "FOUND_REQUEST_TOLERANCE_OFFSET"
  | .deposit => "FOUND_REQUEST_DEPOSIT_OFFSET"
  | .claimUnitAtoms => "FOUND_REQUEST_CLAIM_UNIT_ATOMS_OFFSET"
  | .generation => "FOUND_REQUEST_GENERATION_OFFSET"

def QuoteRequestField.constantName : QuoteRequestField → String
  | .magic => "QUOTE_REQUEST_MAGIC_OFFSET"
  | .version => "QUOTE_REQUEST_VERSION_OFFSET"
  | .reserved => "QUOTE_REQUEST_RESERVED_OFFSET"
  | .market => "QUOTE_REQUEST_MARKET_OFFSET"
  | .dealerId => "QUOTE_REQUEST_DEALER_ID_OFFSET"
  | .expectedFundRevision => "QUOTE_REQUEST_EXPECTED_FUND_REVISION_OFFSET"

def FillRequestField.constantName : FillRequestField → String
  | .magic => "FILL_REQUEST_MAGIC_OFFSET"
  | .version => "FILL_REQUEST_VERSION_OFFSET"
  | .outcomeCount => "FILL_REQUEST_OUTCOME_COUNT_OFFSET"
  | .reserved => "FILL_REQUEST_RESERVED_OFFSET"
  | .market => "FILL_REQUEST_MARKET_OFFSET"
  | .dealerId => "FILL_REQUEST_DEALER_ID_OFFSET"
  | .taker => "FILL_REQUEST_TAKER_OFFSET"
  | .expectedFundRevision => "FILL_REQUEST_EXPECTED_FUND_REVISION_OFFSET"
  | .mint => "FILL_REQUEST_MINT_OFFSET"
  | .prices => "FILL_REQUEST_PRICES_OFFSET"
  | .receive => "FILL_REQUEST_RECEIVE_OFFSET"
  | .deliver => "FILL_REQUEST_DELIVER_OFFSET"

def WithdrawRequestField.constantName : WithdrawRequestField → String
  | .magic => "WITHDRAW_REQUEST_MAGIC_OFFSET"
  | .version => "WITHDRAW_REQUEST_VERSION_OFFSET"
  | .reserved => "WITHDRAW_REQUEST_RESERVED_OFFSET"
  | .market => "WITHDRAW_REQUEST_MARKET_OFFSET"
  | .dealerId => "WITHDRAW_REQUEST_DEALER_ID_OFFSET"
  | .expectedFundRevision => "WITHDRAW_REQUEST_EXPECTED_FUND_REVISION_OFFSET"
  | .amount => "WITHDRAW_REQUEST_AMOUNT_OFFSET"

def ReceiptField.constantName : ReceiptField → String
  | .magic => "RECEIPT_MAGIC_OFFSET"
  | .version => "RECEIPT_VERSION_OFFSET"
  | .route => "RECEIPT_ROUTE_OFFSET"
  | .outcomeCount => "RECEIPT_OUTCOME_COUNT_OFFSET"
  | .reserved => "RECEIPT_RESERVED_OFFSET"
  | .requestDigest => "RECEIPT_REQUEST_DIGEST_OFFSET"
  | .market => "RECEIPT_MARKET_OFFSET"
  | .dealerId => "RECEIPT_DEALER_ID_OFFSET"
  | .fundRevision => "RECEIPT_FUND_REVISION_OFFSET"
  | .cash => "RECEIPT_CASH_OFFSET"
  | .inventoryMinimum => "RECEIPT_INVENTORY_MINIMUM_OFFSET"
  | .liquidityCost => "RECEIPT_LIQUIDITY_COST_OFFSET"
  | .dealerPays => "RECEIPT_DEALER_PAYS_OFFSET"
  | .dealerReceives => "RECEIPT_DEALER_RECEIVES_OFFSET"
  | .fundDigest => "RECEIPT_FUND_DIGEST_OFFSET"

def FillWitnessField.constantName : FillWitnessField → String
  | .magic => "FILL_WITNESS_MAGIC_OFFSET"
  | .version => "FILL_WITNESS_VERSION_OFFSET"
  | .outcomeCount => "FILL_WITNESS_OUTCOME_COUNT_OFFSET"
  | .reserved => "FILL_WITNESS_RESERVED_OFFSET"
  | .rule => "FILL_WITNESS_RULE_OFFSET"
  | .inventory => "FILL_WITNESS_INVENTORY_OFFSET"
  | .fundRevision => "FILL_WITNESS_FUND_REVISION_OFFSET"
  | .cash => "FILL_WITNESS_CASH_OFFSET"

/-- `FOUND_SPONSOR_ACCOUNT`, `FILL_TAKER_TOKEN_ACCOUNT`, ...: a frame slot's
constant name from the frame's prefix and the slot's own name. -/
def slotConstantName (prefix_ : String) (slot : FrameSlot) : String :=
  prefix_ ++ "_" ++ slot.name.toUpper ++ "_ACCOUNT"

/-! ## Corpus -/

example : cashLegs 300 100 0 = ⟨200, 100, 0, 0⟩ := by decide
example : cashLegs 300 400 0 = ⟨0, 300, 100, 0⟩ := by decide
example : cashLegs 300 0 50 = ⟨300, 0, 0, 50⟩ := by decide
example : cashLegs 0 0 50 = ⟨0, 0, 0, 50⟩ := by decide

end DClutch.ScoringRuleAbiV1
