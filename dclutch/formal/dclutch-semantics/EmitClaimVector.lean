import DClutchSemantics.Examples
import DClutchSemantics.Physical

open DClutch

def main : IO Unit :=
  IO.println <| Codec.hex <| Codec.encodePlan <|
    Direct.Physical.physicalPlan Direct.Examples.frame |>.claimEffects
