import DClutchSemantics.AbiSchema

/-!
# Coverage: the half of a layout that disjointness does not state

`specializeFrom_pairwise` says placed fields never overlap and
`specializeFrom_bounded` says none runs past the schema's width.  Neither says
the fields COVER the width.  A record whose readers believe it is 256 bytes and
whose field list accounts for 248 of them has eight bytes with no owner, and a
reserved span is exactly where that hides, because a reserved span looks like an
answer.

`tiles` is the missing statement: the placed fields, in order, begin at the
cursor and end exactly at a declared width, with no gap.  It is `Bool`-valued so
a record discharges it with `decide`, and the fact worth discharging per record
is not the structural half -- sequential placement never leaves a gap -- but that
the width the Rust and the browser DECLARE is the width the fields add up to.
-/

namespace DClutch.AbiSchema

/-- The placed fields tile `[cursor, width)`: each begins where the last ended,
and the last ends exactly at `width`. -/
def tiles {Name : Type} : Nat → List (PlacedField Name) → Nat → Bool
  | cursor, [], width => cursor == width
  | cursor, field :: rest, width =>
      (field.offset == cursor) && tiles (cursor + field.spec.kind.byteWidth) rest width

end DClutch.AbiSchema
