import DClutchSemantics.Examples
import DClutchSemantics.Physical

open DClutch

def main : IO Unit :=
  IO.println <| Codec.hex <| Direct.Physical.Codec.encodeCustodyPlan <|
    Direct.Physical.physicalPlan Direct.Examples.frame |>.custodyTransfers
