import DClutchSemantics.RationalRepresentationV2

open DClutch.RationalRepresentationV2

def denominators : List Nat := [1, 2, 3, 10, 17]
def inputs : List Nat := [0, 1, 2, 3, 9, 10, 11, 19, 20, 37]

def emitCoalescing : IO Unit := do
  for denominator in denominators do
    for input in inputs do
      match coalesce denominator input with
      | none => IO.println s!"C {denominator} {input} 0 0 0"
      | some result =>
          IO.println s!"C {denominator} {input} 1 {result.nativeClaims} {result.changeShards}"

def emitStructured : IO Unit := do
  for denominator in denominators do
    for receiptSupply in [0, 1, 2, 7] do
      for coefficient in [1, 2, 3, 7] do
        for nativeLocked in [0, 1, 3, 8] do
          let shardSupply := denominator * nativeLocked
          let custody := receiptSupply * coefficient
          for free in [0, 1, 2, 9, 11, shardSupply - custody] do
            let coordinate : StructuredCoordinate := {
              coefficient, nativeLocked, shardSupply,
              structuredCustody := custody,
              explicitFreeShards := free
            }
            let valid :=
              0 < denominator &&
              coordinate.shardSupply = denominator * coordinate.nativeLocked &&
              coordinate.structuredCustody = receiptSupply * coordinate.coefficient &&
              coordinate.shardSupply =
                coordinate.structuredCustody + coordinate.explicitFreeShards
            IO.println s!"S {denominator} {receiptSupply} {coefficient} {nativeLocked} {shardSupply} {custody} {free} {if valid then 1 else 0}"
            if valid then
              for quantity in [0, 1, 2, 5] do
                let amount := quantity * coefficient
                let issueAccepted := 0 < quantity && amount ≤ free
                let reconstituteAccepted :=
                  0 < quantity && quantity ≤ nativeLocked && denominator * quantity ≤ free
                IO.println s!"I {denominator} {receiptSupply} {coefficient} {nativeLocked} {shardSupply} {custody} {free} {quantity} {if issueAccepted then 1 else 0}"
                IO.println s!"R {denominator} {receiptSupply} {coefficient} {nativeLocked} {shardSupply} {custody} {free} {quantity} {if reconstituteAccepted then 1 else 0}"

def main : IO Unit := do
  emitCoalescing
  emitStructured
