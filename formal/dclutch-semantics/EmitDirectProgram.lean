import DClutchSemantics.DirectProgram

open DClutch

def main : IO Unit :=
  IO.println <| Codec.hex <| TransitionVM.Codec.encodeProgram DirectProgram.program
