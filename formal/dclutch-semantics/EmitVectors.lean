import DClutchSemantics.Codec
import DClutchSemantics.Examples

open DClutch

def main : IO Unit :=
  IO.println <| Codec.hex <| Codec.encodePlan <|
    Direct.effectPlan Direct.Examples.frame
