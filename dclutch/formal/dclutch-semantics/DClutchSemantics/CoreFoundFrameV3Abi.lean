import DClutchSemantics.AbiSchema

/-!
# The Core Found V3 account frame

Every other record in this tree is a byte layout.  This one is an ACCOUNT
layout: the exact ordered list of accounts a `Found` instruction presents, each
with its writable and signer privileges.  It is a fixed layout in the same sense
a struct is -- a caller that puts the Registry program where the Rent sysvar
belongs is as wrong as one that writes a `u64` two bytes late -- and until now it
had no owner at all.

It had three partial ones instead.
`crates/dclutch-market-core-codec/src/found_frame_v3.rs` held the counts and the
named indices as literals `37`, `28` and `22`, with `const _` asserts relating
them.  `crates/dclutch-product-runtime-v2-operator/src/found.rs` held the
ORDER and the privileges, as a `vec![AccountMeta...]` literal whose own comment
says it must stay one literal because "the SDK and web ABI generators read
`found_metas` by regex".  And the browser held the human LABELS -- a
thirty-nine-entry dictionary in `generate-core-found.mjs`, a table of protocol
facts whose only author was a JavaScript object, with nothing in Rust or Lean
against which a wrong label could be checked.

A frame is a list of slots.  The counts are its length, the named indices are
positions in it, the privileges are its slots' fields, and the labels ride
along, so a slot cannot be renamed in one place and not another.
-/

namespace DClutch.CoreFoundFrameV3Abi

/-- One account in the frame: the operator's own field path, the browser's
label, and the two privileges the runtime checks. -/
structure Slot where
  /-- The `state.…` path `found_metas` projects, verbatim. -/
  field : String
  /-- The label a reader is shown for this position. -/
  label : String
  writable : Bool
  signer : Bool
  deriving DecidableEq, Repr

private def ro (field label : String) : Slot :=
  { field, label, writable := false, signer := false }

/-- The canonical mutating Found V3 frame, in order. -/
def canonicalFrame : List Slot := [
  { field := "state.payer", label := "payer", writable := true, signer := true },
  { field := "state.market", label := "Market destination", writable := true, signer := false },
  ro "state.rent_credit" "RentCredit",
  ro "state.rent_program" "Rent program",
  ro "state.realm.record.raw" "Realm raw",
  ro "state.realm.record.staging" "Realm staging",
  ro "state.product.raw" "Product raw",
  ro "state.product.staging" "Product staging",
  ro "state.result_domain.raw" "result domain raw",
  ro "state.result_domain.staging" "result domain staging",
  ro "state.portfolio.raw" "portfolio raw",
  ro "state.portfolio.staging" "portfolio staging",
  ro "state.linked_basis.raw" "linked basis raw",
  ro "state.linked_basis.staging" "linked basis staging",
  ro "state.source_material.record.raw" "Source material raw",
  ro "state.source_material.record.staging" "Source staging",
  ro "state.source_spec.record.raw" "Source spec raw",
  ro "state.source_spec.record.staging" "Source spec staging",
  ro "state.capacity_profile.record.raw" "capacity profile raw",
  ro "state.capacity_profile.record.staging" "capacity profile staging",
  ro "state.manipulation_floor.record.raw" "manipulation floor raw",
  ro "state.manipulation_floor.record.staging" "manipulation floor staging",
  ro "state.capability_manifest.record.raw" "capability manifest raw",
  ro "state.capability_manifest.record.staging" "capability staging",
  ro "state.activation_cache" "activation cache",
  ro "state.core_program" "Core program",
  ro "state.core_programdata" "Core ProgramData",
  ro "state.registry_program" "Registry program",
  ro "state.rent" "Rent sysvar",
  ro "state.system_program" "System program",
  ro "state.infrastructure_profile" "infrastructure profile",
  ro "state.registry_artifact.raw" "Registry artifact raw",
  ro "state.registry_artifact.staging" "Registry artifact staging",
  ro "state.registry_programdata" "Registry ProgramData",
  ro "state.rent_artifact.raw" "Rent artifact raw",
  ro "state.rent_artifact.staging" "Rent artifact staging",
  ro "state.rent_programdata" "Rent ProgramData"
]

/-- The optional no-arbitrage certificate pair, APPENDED.  A `DCLTPGT1`
certificate is required exactly when the basis declares a degree at or above
two, which is the spline family and nothing else; every categorical and graded
founding carries none.  Appending rather than inserting is what keeps every
existing coordinate and every existing caller unchanged. -/
def priceGateExtension : List Slot := [
  ro "certificate.raw" "price-gate certificate raw",
  ro "certificate.staging" "price-gate certificate staging"
]

def extendedFrame : List Slot := canonicalFrame ++ priceGateExtension

def accountCount : Nat := canonicalFrame.length
def priceGateAccountCount : Nat := extendedFrame.length

/-- Position of a slot by the operator's own field path. -/
def indexOf? (field : String) : Option Nat :=
  extendedFrame.findIdx? (fun slot => slot.field == field)

def rentSysvarIndex : Nat := (indexOf? "state.rent").getD 0
def capabilityManifestRawIndex : Nat :=
  (indexOf? "state.capability_manifest.record.raw").getD 0
def priceGateRawIndex : Nat := (indexOf? "certificate.raw").getD 0
def priceGateStagingIndex : Nat := (indexOf? "certificate.staging").getD 0
def capabilityManifestStagingIndex : Nat :=
  (indexOf? "state.capability_manifest.record.staging").getD 0

/-! ## What the frame says -/

/-- The three numbers `found_frame_v3.rs` wrote as literals are positions in the
list the operator builds.  A slot inserted anywhere before the Rent sysvar moves
this index, which is the whole point: it could not, before. -/
theorem named_indices_are_positions :
    accountCount = 37 ∧ priceGateAccountCount = 39 ∧
    rentSysvarIndex = 28 ∧ capabilityManifestRawIndex = 22 ∧
    priceGateRawIndex = 37 ∧ priceGateStagingIndex = 38 ∧
    capabilityManifestStagingIndex = 23 := by
  native_decide

/-- Every named coordinate the frame module asserts, restated as facts about the
list rather than as arithmetic between literals. -/
theorem named_indices_are_admissible :
    capabilityManifestStagingIndex = capabilityManifestRawIndex + 1 ∧
    capabilityManifestStagingIndex < accountCount ∧
    rentSysvarIndex < accountCount ∧
    priceGateRawIndex = accountCount ∧
    priceGateStagingIndex = priceGateRawIndex + 1 ∧
    priceGateStagingIndex < priceGateAccountCount ∧
    priceGateRawIndex > rentSysvarIndex := by
  native_decide

/-- Exactly one account signs, and it is the payer.  Nothing stated this before:
the privileges lived one per line in a `vec!` literal, where "how many signers
does a Found present" is a question you answer by counting. -/
theorem the_payer_is_the_only_signer :
    extendedFrame.filter (fun slot => slot.signer) =
      [{ field := "state.payer", label := "payer", writable := true, signer := true }] := by
  native_decide

/-- Exactly two accounts are writable: the payer that funds the founding and the
Market being created.  Every record, program, sysvar and cursor in the frame is
readonly. -/
theorem only_the_payer_and_the_market_are_writable :
    (extendedFrame.filter (fun slot => slot.writable)).map (fun slot => slot.field) =
      ["state.payer", "state.market"] := by
  native_decide

/-- A signer is writable here, so no slot can be a signer without also being
declared writable by accident of ordering. -/
theorem every_signer_is_writable :
    extendedFrame.all (fun slot => !slot.signer || slot.writable) := by
  native_decide

/-- No field path and no label is used twice, so `indexOf?` names one position
and a reader shown two identical labels is looking at a defect. -/
theorem slots_are_uniquely_named :
    (extendedFrame.map (fun slot => slot.field)).Nodup ∧
    (extendedFrame.map (fun slot => slot.label)).Nodup := by
  native_decide

/-- The certificate pair is strictly appended: the canonical frame is a prefix
of the extended one, so no existing coordinate moved and no caller that never
founds curvature changes. -/
theorem the_extension_is_a_suffix :
    extendedFrame.take accountCount = canonicalFrame ∧
    extendedFrame.drop accountCount = priceGateExtension := by
  native_decide

end DClutch.CoreFoundFrameV3Abi
