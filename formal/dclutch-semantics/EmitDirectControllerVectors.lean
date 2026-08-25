import DClutchSemantics.DirectControllerCodec

open DClutch.Codec
open DClutch.DirectControllerCodec
open DClutch.DirectControllerCodec.Examples

def main : IO Unit := do
  IO.println s!"seller_intent={hex (encodeCompactIntentV1 sellerIntent)}"
  IO.println s!"buyer_intent={hex (encodeCompactIntentV1 buyerIntent)}"
  IO.println s!"controller={hex (encodeControllerInstructionV1 controllerInstruction)}"
  IO.println s!"market_profile={hex (encodeMarketProfileV1 marketProfile)}"
