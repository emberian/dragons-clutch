import DClutchSemantics.EconomicCodec

open DClutch
open DClutch.Economic

def fixtureLines (name : String) (frame : Frame) : List String := [
  s!"{name}.pre={Codec.hex <| Economic.Codec.encodeState frame.outcomeCount frame.pre}",
  s!"{name}.post={Codec.hex <| Economic.Codec.encodeState frame.outcomeCount (runState frame)}",
  s!"{name}.claims={Codec.hex <| Economic.Codec.encodeClaimPlan frame}",
  s!"{name}.custody={Codec.hex <| Economic.Codec.encodeCustodyPlan frame}"
]

def main : IO Unit :=
  IO.println <| String.intercalate "\n" <| List.flatten [
    fixtureLines "split" Economic.Examples.splitFrame,
    fixtureLines "merge" Economic.Examples.mergeFrame,
    fixtureLines "transfer" Economic.Examples.transferFrame,
    fixtureLines "materialize" Economic.Examples.materializeFrame,
    fixtureLines "dematerialize" Economic.Examples.dematerializeFrame,
    fixtureLines "redeem_winner" Economic.Examples.redeemWinner,
    fixtureLines "redeem_loser" Economic.Examples.redeemLoser,
    fixtureLines "retire" Economic.Examples.retireEmpty
  ]
