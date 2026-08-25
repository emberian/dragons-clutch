/-!
# Typed fixed-layout ABI schemas

This module is the first-class schema layer between protocol semantics and a
concrete byte ABI.  A schema is data: it names fields and assigns each one a
wire kind.  `specialize` is the only owner of field offsets; it places fields
left-to-right from a cursor and therefore cannot create overlap or drift
between separately maintained offset tables.

The Direct examples at the end reproduce the current 136-byte signed intent
and 304-byte controller envelope.  They include fixed magic, version and
reserved spans so future encoders and hostile decoders can be derived from the
same complete description rather than sharing only the dynamic fields.
-/

namespace DClutch.AbiSchema

/-- Wire kinds supported by the first fixed-layout specializer.  `bytes` is an
opaque fixed-width coordinate, `reserved` must be canonically zero on the
wire, and `nested` embeds another already-specialized fixed-width schema. -/
inductive FieldKind where
  | u8
  | u16
  | u32
  | u64
  | bytes (width : Nat)
  | reserved (width : Nat)
  | nested (width : Nat)
  deriving DecidableEq, Repr

namespace FieldKind

/-- Exact physical width of a wire kind. -/
def byteWidth : FieldKind → Nat
  | .u8 => 1
  | .u16 => 2
  | .u32 => 4
  | .u64 => 8
  | .bytes width | .reserved width | .nested width => width

end FieldKind

/-- A semantic field name paired with its physical representation. -/
structure FieldSpec (Name : Type) where
  name : Name
  kind : FieldKind
  deriving DecidableEq, Repr

/-- A field after specialization.  Offsets are deliberately absent from
`FieldSpec`; only this result type can carry one. -/
structure PlacedField (Name : Type) where
  spec : FieldSpec Name
  offset : Nat
  deriving DecidableEq, Repr

/-- Total width of a schema, independent of the cursor at which it is placed. -/
def schemaWidth {Name : Type} : List (FieldSpec Name) → Nat
  | [] => 0
  | field :: rest => field.kind.byteWidth + schemaWidth rest

/-- Place every field sequentially, starting at `cursor`. -/
def specializeFrom {Name : Type} : Nat → List (FieldSpec Name) → List (PlacedField Name)
  | _, [] => []
  | cursor, field :: rest =>
      { spec := field, offset := cursor } ::
        specializeFrom (cursor + field.kind.byteWidth) rest

/-- Canonical zero-based ABI layout. -/
def specialize {Name : Type} (schema : List (FieldSpec Name)) : List (PlacedField Name) :=
  specializeFrom 0 schema

/-- Compact projection useful to generators, decoders and differential tests. -/
def coordinates {Name : Type} (layout : List (PlacedField Name)) :
    List (Name × Nat × Nat) :=
  layout.map fun field => (field.spec.name, field.offset, field.spec.kind.byteWidth)

/-- Named lookup for generators and hostile decoders.  It returns the derived
offset and width together so a consumer cannot accidentally combine an offset
from one field with the width of another. -/
def coordinate? {Name : Type} [DecidableEq Name] (name : Name) :
    List (PlacedField Name) → Option (Nat × Nat)
  | [] => none
  | field :: rest =>
      if field.spec.name = name then
        some (field.offset, field.spec.kind.byteWidth)
      else
        coordinate? name rest

/-- Minimum conditions for a public fixed-layout ABI: one semantic owner for
each field name and no empty fields sharing a cursor. -/
def WellFormed {Name : Type} (schema : List (FieldSpec Name)) : Prop :=
  (schema.map fun field => field.name).Nodup ∧
    ∀ field ∈ schema, 0 < field.kind.byteWidth

/-- Two fields are ordered and byte-disjoint when the left field ends no later
than the right field begins. -/
def Before {Name : Type} (left right : PlacedField Name) : Prop :=
  left.offset + left.spec.kind.byteWidth ≤ right.offset

theorem specializeFrom_length {Name : Type} (cursor : Nat)
    (schema : List (FieldSpec Name)) :
    (specializeFrom cursor schema).length = schema.length := by
  induction schema generalizing cursor with
  | nil => rfl
  | cons field rest induction =>
      simp [specializeFrom, induction]

theorem specializeFrom_names {Name : Type} (cursor : Nat)
    (schema : List (FieldSpec Name)) :
    (specializeFrom cursor schema).map (fun field => field.spec.name) =
      schema.map (fun field => field.name) := by
  induction schema generalizing cursor with
  | nil => rfl
  | cons field rest induction =>
      simp [specializeFrom, induction]

/-- Every specialized offset is at or beyond its starting cursor. -/
theorem offset_ge_cursor {Name : Type} (cursor : Nat)
    (schema : List (FieldSpec Name)) (placed : PlacedField Name)
    (member : placed ∈ specializeFrom cursor schema) :
    cursor ≤ placed.offset := by
  induction schema generalizing cursor with
  | nil => simp [specializeFrom] at member
  | cons field rest induction =>
      simp only [specializeFrom, List.mem_cons] at member
      rcases member with rfl | member
      · exact Nat.le_refl cursor
      · exact Nat.le_trans (Nat.le_add_right cursor field.kind.byteWidth)
          (induction (cursor := cursor + field.kind.byteWidth) member)

/-- Every specialized field ends within the schema's computed final cursor. -/
theorem specializeFrom_bounded {Name : Type} (cursor : Nat)
    (schema : List (FieldSpec Name)) (placed : PlacedField Name)
    (member : placed ∈ specializeFrom cursor schema) :
    placed.offset + placed.spec.kind.byteWidth ≤ cursor + schemaWidth schema := by
  induction schema generalizing cursor with
  | nil => simp [specializeFrom] at member
  | cons field rest induction =>
      simp only [specializeFrom, List.mem_cons] at member
      rcases member with rfl | member
      · simp [schemaWidth]
      · have bounded := induction
          (cursor := cursor + field.kind.byteWidth) member
        simpa [schemaWidth, Nat.add_assoc] using bounded

/-- Sequential specialization makes every pair of fields ordered and
byte-disjoint.  This is structural; it requires no per-schema arithmetic
audit. -/
theorem specializeFrom_pairwise {Name : Type} (cursor : Nat)
    (schema : List (FieldSpec Name)) :
    (specializeFrom cursor schema).Pairwise Before := by
  induction schema generalizing cursor with
  | nil => simp [specializeFrom]
  | cons field rest induction =>
      rw [specializeFrom]
      apply List.pairwise_cons.2
      constructor
      · intro placed member
        exact offset_ge_cursor (cursor + field.kind.byteWidth) rest placed member
      · exact induction (cursor := cursor + field.kind.byteWidth)

/-- Splitting a schema does not change any offset: the suffix begins exactly
at the prefix's computed end.  This is the compositional/canonical-offset law
used when a nested or capability-specific schema is assembled from pieces. -/
theorem specializeFrom_append {Name : Type} (cursor : Nat)
    (leading trailing : List (FieldSpec Name)) :
    specializeFrom cursor (leading ++ trailing) =
      specializeFrom cursor leading ++
        specializeFrom (cursor + schemaWidth leading) trailing := by
  induction leading generalizing cursor with
  | nil => simp [specializeFrom, schemaWidth]
  | cons field rest induction =>
      simp only [List.cons_append, specializeFrom, schemaWidth, List.cons.injEq,
        true_and]
      rw [induction]
      simp [Nat.add_assoc]

/-- A field following an arbitrary prefix is placed at precisely the prefix
width.  This gives every generated offset a unique data-derived explanation. -/
theorem field_after_prefix {Name : Type} (cursor : Nat)
    (leading trailing : List (FieldSpec Name)) (field : FieldSpec Name) :
    { spec := field, offset := cursor + schemaWidth leading } ∈
      specializeFrom cursor (leading ++ field :: trailing) := by
  rw [specializeFrom_append]
  simp [specializeFrom]

/-- Specialization preserves unique semantic ownership of field names. -/
theorem specializeFrom_names_nodup {Name : Type} (cursor : Nat)
    (schema : List (FieldSpec Name))
    (unique : (schema.map fun field => field.name).Nodup) :
    ((specializeFrom cursor schema).map fun field => field.spec.name).Nodup := by
  rw [specializeFrom_names]
  exact unique

/-- Named lookup agrees with the compositional prefix law.  The uniqueness
hypothesis rules out an earlier field with the same semantic name. -/
theorem coordinate_field_after_prefix {Name : Type} [DecidableEq Name]
    (cursor : Nat) (leading trailing : List (FieldSpec Name))
    (field : FieldSpec Name)
    (absent : ∀ candidate ∈ leading, candidate.name ≠ field.name) :
    coordinate? field.name
        (specializeFrom cursor (leading ++ field :: trailing)) =
      some (cursor + schemaWidth leading, field.kind.byteWidth) := by
  induction leading generalizing cursor with
  | nil => simp [specializeFrom, coordinate?, schemaWidth]
  | cons head rest induction =>
      have headDifferent : head.name ≠ field.name :=
        absent head (by simp)
      have restAbsent : ∀ candidate ∈ rest, candidate.name ≠ field.name := by
        intro candidate member
        exact absent candidate (by simp [member])
      simp [specializeFrom, coordinate?, headDifferent,
        induction (cursor := cursor + head.kind.byteWidth) restAbsent,
        schemaWidth, Nat.add_assoc]

/-! ## Current signed Direct ABI as schema data -/

inductive DirectIntentField where
  | magic | version | side | outcome | lifecycle | reservedA | market
  | generation | nonce | validFrom | validThrough | maximumFill | limitPrice
  | feeBasisPoints | reservedB | collateralAccount
  deriving DecidableEq, Repr

def directIntentSchema : List (FieldSpec DirectIntentField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.side, .u8⟩,
  ⟨.outcome, .u8⟩,
  ⟨.lifecycle, .u8⟩,
  ⟨.reservedA, .reserved 3⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.nonce, .u64⟩,
  ⟨.validFrom, .u64⟩,
  ⟨.validThrough, .u64⟩,
  ⟨.maximumFill, .u64⟩,
  ⟨.limitPrice, .u64⟩,
  ⟨.feeBasisPoints, .u16⟩,
  ⟨.reservedB, .reserved 6⟩,
  ⟨.collateralAccount, .bytes 32⟩
]

def directIntentLayout : List (PlacedField DirectIntentField) :=
  specialize directIntentSchema

theorem directIntentSchema_width : schemaWidth directIntentSchema = 136 := by
  native_decide

theorem directIntentSchema_unique_names :
    (directIntentSchema.map fun field => field.name).Nodup := by
  native_decide

theorem directIntentSchema_wellFormed : WellFormed directIntentSchema := by
  constructor
  · native_decide
  · intro field member
    simp [directIntentSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
      decide

/-- Regression witness for every current signed-intent offset.  This is a
comparison against the intended public ABI, not a second offset definition used
by the specializer. -/
theorem directIntentCoordinates : coordinates directIntentLayout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.side, 10, 1),
    (.outcome, 11, 1),
    (.lifecycle, 12, 1),
    (.reservedA, 13, 3),
    (.market, 16, 32),
    (.generation, 48, 8),
    (.nonce, 56, 8),
    (.validFrom, 64, 8),
    (.validThrough, 72, 8),
    (.maximumFill, 80, 8),
    (.limitPrice, 88, 8),
    (.feeBasisPoints, 96, 2),
    (.reservedB, 98, 6),
    (.collateralAccount, 104, 32)
  ] := by
  native_decide

theorem directIntentFields_bounded (placed : PlacedField DirectIntentField)
    (member : placed ∈ directIntentLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ 136 := by
  simpa [directIntentLayout, specialize, directIntentSchema_width] using
    specializeFrom_bounded 0 directIntentSchema placed member

theorem directIntentFields_disjoint : directIntentLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 directIntentSchema

theorem directIntentMarketCoordinate :
    coordinate? .market directIntentLayout = some (16, 32) := by
  native_decide

theorem directIntentCollateralCoordinate :
    coordinate? .collateralAccount directIntentLayout = some (104, 32) := by
  native_decide

inductive DirectControllerField where
  | magic | version | controllerBump | sellerReplayBump | buyerReplayBump
  | sellerPositionBump | buyerPositionBump | reserved | fill | executionPrice
  | seller | buyer
  deriving DecidableEq, Repr

def directControllerSchema : List (FieldSpec DirectControllerField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.controllerBump, .u8⟩,
  ⟨.sellerReplayBump, .u8⟩,
  ⟨.buyerReplayBump, .u8⟩,
  ⟨.sellerPositionBump, .u8⟩,
  ⟨.buyerPositionBump, .u8⟩,
  ⟨.reserved, .reserved 1⟩,
  ⟨.fill, .u64⟩,
  ⟨.executionPrice, .u64⟩,
  ⟨.seller, .nested (schemaWidth directIntentSchema)⟩,
  ⟨.buyer, .nested (schemaWidth directIntentSchema)⟩
]

def directControllerLayout : List (PlacedField DirectControllerField) :=
  specialize directControllerSchema

theorem directControllerSchema_width : schemaWidth directControllerSchema = 304 := by
  native_decide

theorem directControllerSchema_unique_names :
    (directControllerSchema.map fun field => field.name).Nodup := by
  native_decide

theorem directControllerSchema_wellFormed : WellFormed directControllerSchema := by
  constructor
  · native_decide
  · intro field member
    simp [directControllerSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl <;>
      native_decide

theorem directControllerCoordinates : coordinates directControllerLayout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.controllerBump, 10, 1),
    (.sellerReplayBump, 11, 1),
    (.buyerReplayBump, 12, 1),
    (.sellerPositionBump, 13, 1),
    (.buyerPositionBump, 14, 1),
    (.reserved, 15, 1),
    (.fill, 16, 8),
    (.executionPrice, 24, 8),
    (.seller, 32, 136),
    (.buyer, 168, 136)
  ] := by
  native_decide

theorem directControllerFields_bounded
    (placed : PlacedField DirectControllerField)
    (member : placed ∈ directControllerLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ 304 := by
  simpa [directControllerLayout, specialize, directControllerSchema_width] using
    specializeFrom_bounded 0 directControllerSchema placed member

theorem directControllerFields_disjoint :
    directControllerLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 directControllerSchema

theorem directControllerSellerCoordinate :
    coordinate? .seller directControllerLayout = some (32, 136) := by
  native_decide

theorem directControllerBuyerCoordinate :
    coordinate? .buyer directControllerLayout = some (168, 136) := by
  native_decide

end DClutch.AbiSchema
