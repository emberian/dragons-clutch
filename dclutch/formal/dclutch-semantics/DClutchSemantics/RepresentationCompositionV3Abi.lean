import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Representation composition V3 fixed ABI and flattening correspondence

This module owns the fixed physical coordinates and a bounded mathematical
model of the canonical representation-composition DAG.  It deliberately does
not implement the hostile decoder or runtime adapter: the safe, `no_std` Rust
kernel remains the executable semantic owner, and finalized-record admission,
SHA-256, Product/Core observations, Claims, Token, and Custody remain outside
this model's trust boundary.

The checked arithmetic model uses the Rust capacity profile's `u128` ceiling.
Those bounds are an executable profile, not a restriction on the protocol
ontology.  Generated Rust contains only constants and canonical/hostile witness
bytes used for translation tests.
-/

namespace DClutch.RepresentationCompositionV3Abi

open DClutch.AbiSchema

def schemaVersion : Nat := 3
def minOutcomes : Nat := 2
def maxOutcomes : Nat := 256
def maxNodes : Nat := 32
def maxEdges : Nat := 96
def maxTerms : Nat := 2048
def maxU128 : Nat := 2 ^ 128 - 1

def capacityProfilePreimage : List UInt8 :=
  "dclutch/capacity/representation-composition-v3/outcomes256/nodes32/edges96/terms2048/u128".toUTF8.toList
def capacityProfileId : List UInt8 := [
  0x48, 0xaa, 0xa1, 0xf4, 0x37, 0xff, 0xda, 0xc9,
  0xbf, 0x14, 0xc9, 0xd8, 0xc8, 0xc4, 0x9c, 0xf3,
  0xf7, 0x1e, 0x93, 0x9e, 0x30, 0x39, 0x79, 0x4b,
  0xf7, 0xc4, 0x11, 0xa8, 0xff, 0x8d, 0xb8, 0x78
]

def descriptorSchemaPreimage : List UInt8 :=
  "dclutch/schema/representation-composition-descriptor-v3".toUTF8.toList
def descriptorSchemaId : List UInt8 := [
  0xfa, 0x76, 0x41, 0xfb, 0x0c, 0x60, 0xc1, 0x74,
  0xe4, 0x7a, 0x45, 0x69, 0x99, 0x6a, 0xcc, 0x5d,
  0x12, 0x6a, 0x6c, 0x6d, 0xb7, 0xb4, 0xa5, 0xa9,
  0x2f, 0x23, 0x86, 0xb5, 0x49, 0xd9, 0x12, 0x88
]
def graphSchemaPreimage : List UInt8 :=
  "dclutch/schema/representation-composition-graph-v3".toUTF8.toList
def graphSchemaId : List UInt8 := [
  0xb3, 0xc5, 0xc7, 0x7b, 0x58, 0x0a, 0x29, 0x6d,
  0xf5, 0xf7, 0x59, 0x70, 0x4b, 0x99, 0x9b, 0xfb,
  0x79, 0xc6, 0xc2, 0x39, 0x6c, 0x4c, 0x39, 0xb2,
  0xf4, 0xc5, 0x78, 0xc8, 0x72, 0x11, 0x57, 0x84
]
def translationSchemaPreimage : List UInt8 :=
  "dclutch/schema/representation-composition-translation-v3".toUTF8.toList
def translationSchemaId : List UInt8 := [
  0xd2, 0xc1, 0x0c, 0x1f, 0xe6, 0xd8, 0xfc, 0x09,
  0x42, 0x10, 0xca, 0xad, 0x45, 0xd7, 0x00, 0x34,
  0x76, 0xe5, 0x98, 0x8b, 0xe5, 0xa0, 0x69, 0xe8,
  0x0c, 0x71, 0xec, 0x30, 0x0c, 0x2a, 0xe6, 0x41
]

def descriptorMagic : List UInt8 := "DCRCDS03".toUTF8.toList
def graphMagic : List UInt8 := "DCRCDG03".toUTF8.toList
def translationMagic : List UInt8 := "DCRCDT03".toUTF8.toList

inductive DescriptorField where
  | magic | version | reservedHeader | market | resultDomain | releaseSet
  | nativeBasis | graphId | graphDigest | rootId | translationId
  | translationDigest | capacityProfile | outcomeCount | nodeCount
  | edgeCount | termCount | rootDenominator | reservedTail
  deriving DecidableEq, Repr

def descriptorSchema : List (FieldSpec DescriptorField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.market, .bytes 32⟩, ⟨.resultDomain, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.nativeBasis, .bytes 32⟩,
  ⟨.graphId, .bytes 32⟩, ⟨.graphDigest, .bytes 32⟩,
  ⟨.rootId, .bytes 32⟩, ⟨.translationId, .bytes 32⟩,
  ⟨.translationDigest, .bytes 32⟩, ⟨.capacityProfile, .bytes 32⟩,
  ⟨.outcomeCount, .u32⟩, ⟨.nodeCount, .u32⟩, ⟨.edgeCount, .u32⟩,
  ⟨.termCount, .u32⟩, ⟨.rootDenominator, .u64⟩,
  ⟨.reservedTail, .reserved 8⟩
]

inductive GraphHeaderField where
  | magic | version | reservedHeader | graphId | rootId | outcomeCount
  | nodeCount | edgeCount | termCount | rootIndex | reservedTail
  deriving DecidableEq, Repr

def graphHeaderSchema : List (FieldSpec GraphHeaderField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.graphId, .bytes 32⟩, ⟨.rootId, .bytes 32⟩,
  ⟨.outcomeCount, .u32⟩, ⟨.nodeCount, .u32⟩,
  ⟨.edgeCount, .u32⟩, ⟨.termCount, .u32⟩, ⟨.rootIndex, .u32⟩,
  ⟨.reservedTail, .reserved 12⟩
]

inductive NodeField where
  | id | rank | firstEdge | edgeCount | firstTerm | termCount | kind
  | reservedKind | nativeOutcome | reservedScalar | recipeDivisor
  | flattenedDenominator
  deriving DecidableEq, Repr

def nodeSchema : List (FieldSpec NodeField) := [
  ⟨.id, .bytes 32⟩, ⟨.rank, .u32⟩, ⟨.firstEdge, .u32⟩,
  ⟨.edgeCount, .u32⟩, ⟨.firstTerm, .u32⟩, ⟨.termCount, .u32⟩,
  ⟨.kind, .u8⟩, ⟨.reservedKind, .reserved 3⟩, ⟨.nativeOutcome, .u32⟩,
  ⟨.reservedScalar, .reserved 4⟩, ⟨.recipeDivisor, .u64⟩,
  ⟨.flattenedDenominator, .u64⟩
]

inductive EdgeField where
  | childId | childIndex | reserved | coefficient
  deriving DecidableEq, Repr

def edgeSchema : List (FieldSpec EdgeField) := [
  ⟨.childId, .bytes 32⟩, ⟨.childIndex, .u32⟩,
  ⟨.reserved, .reserved 4⟩, ⟨.coefficient, .u64⟩
]

inductive TermField where | outcome | reserved | numerator deriving DecidableEq, Repr

def termSchema : List (FieldSpec TermField) := [
  ⟨.outcome, .u32⟩, ⟨.reserved, .reserved 4⟩, ⟨.numerator, .u64⟩
]

inductive TranslationHeaderField where
  | magic | version | reservedHeader | graphId | rootId | outcomeCount
  | termCount | denominator | reservedTail
  deriving DecidableEq, Repr

def translationHeaderSchema : List (FieldSpec TranslationHeaderField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.graphId, .bytes 32⟩, ⟨.rootId, .bytes 32⟩,
  ⟨.outcomeCount, .u32⟩, ⟨.termCount, .u32⟩,
  ⟨.denominator, .u64⟩, ⟨.reservedTail, .reserved 32⟩
]

def descriptorLayout := specialize descriptorSchema
def graphHeaderLayout := specialize graphHeaderSchema
def nodeLayout := specialize nodeSchema
def edgeLayout := specialize edgeSchema
def termLayout := specialize termSchema
def translationHeaderLayout := specialize translationHeaderSchema

def descriptorBytes := schemaWidth descriptorSchema
def graphHeaderBytes := schemaWidth graphHeaderSchema
def nodeBytes := schemaWidth nodeSchema
def edgeBytes := schemaWidth edgeSchema
def termBytes := schemaWidth termSchema
def translationHeaderBytes := schemaWidth translationHeaderSchema

namespace DescriptorField
def rustName : DescriptorField → String
  | .magic => "COMPOSITION_DESCRIPTOR_MAGIC_OFFSET_V3"
  | .version => "COMPOSITION_DESCRIPTOR_VERSION_OFFSET_V3"
  | .reservedHeader => "COMPOSITION_DESCRIPTOR_RESERVED_HEADER_OFFSET_V3"
  | .market => "COMPOSITION_DESCRIPTOR_MARKET_OFFSET_V3"
  | .resultDomain => "COMPOSITION_DESCRIPTOR_RESULT_DOMAIN_OFFSET_V3"
  | .releaseSet => "COMPOSITION_DESCRIPTOR_RELEASE_SET_OFFSET_V3"
  | .nativeBasis => "COMPOSITION_DESCRIPTOR_NATIVE_BASIS_OFFSET_V3"
  | .graphId => "COMPOSITION_DESCRIPTOR_GRAPH_ID_OFFSET_V3"
  | .graphDigest => "COMPOSITION_DESCRIPTOR_GRAPH_DIGEST_OFFSET_V3"
  | .rootId => "COMPOSITION_DESCRIPTOR_ROOT_ID_OFFSET_V3"
  | .translationId => "COMPOSITION_DESCRIPTOR_TRANSLATION_ID_OFFSET_V3"
  | .translationDigest => "COMPOSITION_DESCRIPTOR_TRANSLATION_DIGEST_OFFSET_V3"
  | .capacityProfile => "COMPOSITION_DESCRIPTOR_CAPACITY_PROFILE_OFFSET_V3"
  | .outcomeCount => "COMPOSITION_DESCRIPTOR_OUTCOME_COUNT_OFFSET_V3"
  | .nodeCount => "COMPOSITION_DESCRIPTOR_NODE_COUNT_OFFSET_V3"
  | .edgeCount => "COMPOSITION_DESCRIPTOR_EDGE_COUNT_OFFSET_V3"
  | .termCount => "COMPOSITION_DESCRIPTOR_TERM_COUNT_OFFSET_V3"
  | .rootDenominator => "COMPOSITION_DESCRIPTOR_ROOT_DENOMINATOR_OFFSET_V3"
  | .reservedTail => "COMPOSITION_DESCRIPTOR_RESERVED_TAIL_OFFSET_V3"
end DescriptorField

namespace GraphHeaderField
def rustName : GraphHeaderField → String
  | .magic => "COMPOSITION_GRAPH_MAGIC_OFFSET_V3"
  | .version => "COMPOSITION_GRAPH_VERSION_OFFSET_V3"
  | .reservedHeader => "COMPOSITION_GRAPH_RESERVED_HEADER_OFFSET_V3"
  | .graphId => "COMPOSITION_GRAPH_ID_OFFSET_V3"
  | .rootId => "COMPOSITION_GRAPH_ROOT_ID_OFFSET_V3"
  | .outcomeCount => "COMPOSITION_GRAPH_OUTCOME_COUNT_OFFSET_V3"
  | .nodeCount => "COMPOSITION_GRAPH_NODE_COUNT_OFFSET_V3"
  | .edgeCount => "COMPOSITION_GRAPH_EDGE_COUNT_OFFSET_V3"
  | .termCount => "COMPOSITION_GRAPH_TERM_COUNT_OFFSET_V3"
  | .rootIndex => "COMPOSITION_GRAPH_ROOT_INDEX_OFFSET_V3"
  | .reservedTail => "COMPOSITION_GRAPH_RESERVED_TAIL_OFFSET_V3"
end GraphHeaderField

namespace NodeField
def rustName : NodeField → String
  | .id => "COMPOSITION_NODE_ID_OFFSET_V3"
  | .rank => "COMPOSITION_NODE_RANK_OFFSET_V3"
  | .firstEdge => "COMPOSITION_NODE_FIRST_EDGE_OFFSET_V3"
  | .edgeCount => "COMPOSITION_NODE_EDGE_COUNT_OFFSET_V3"
  | .firstTerm => "COMPOSITION_NODE_FIRST_TERM_OFFSET_V3"
  | .termCount => "COMPOSITION_NODE_TERM_COUNT_OFFSET_V3"
  | .kind => "COMPOSITION_NODE_KIND_OFFSET_V3"
  | .reservedKind => "COMPOSITION_NODE_RESERVED_KIND_OFFSET_V3"
  | .nativeOutcome => "COMPOSITION_NODE_NATIVE_OUTCOME_OFFSET_V3"
  | .reservedScalar => "COMPOSITION_NODE_RESERVED_SCALAR_OFFSET_V3"
  | .recipeDivisor => "COMPOSITION_NODE_RECIPE_DIVISOR_OFFSET_V3"
  | .flattenedDenominator => "COMPOSITION_NODE_FLATTENED_DENOMINATOR_OFFSET_V3"
end NodeField

namespace EdgeField
def rustName : EdgeField → String
  | .childId => "COMPOSITION_EDGE_CHILD_ID_OFFSET_V3"
  | .childIndex => "COMPOSITION_EDGE_CHILD_INDEX_OFFSET_V3"
  | .reserved => "COMPOSITION_EDGE_RESERVED_OFFSET_V3"
  | .coefficient => "COMPOSITION_EDGE_COEFFICIENT_OFFSET_V3"
end EdgeField

namespace TermField
def rustName : TermField → String
  | .outcome => "COMPOSITION_TERM_OUTCOME_OFFSET_V3"
  | .reserved => "COMPOSITION_TERM_RESERVED_OFFSET_V3"
  | .numerator => "COMPOSITION_TERM_NUMERATOR_OFFSET_V3"
end TermField

namespace TranslationHeaderField
def rustName : TranslationHeaderField → String
  | .magic => "COMPOSITION_TRANSLATION_MAGIC_OFFSET_V3"
  | .version => "COMPOSITION_TRANSLATION_VERSION_OFFSET_V3"
  | .reservedHeader => "COMPOSITION_TRANSLATION_RESERVED_HEADER_OFFSET_V3"
  | .graphId => "COMPOSITION_TRANSLATION_GRAPH_ID_OFFSET_V3"
  | .rootId => "COMPOSITION_TRANSLATION_ROOT_ID_OFFSET_V3"
  | .outcomeCount => "COMPOSITION_TRANSLATION_OUTCOME_COUNT_OFFSET_V3"
  | .termCount => "COMPOSITION_TRANSLATION_TERM_COUNT_OFFSET_V3"
  | .denominator => "COMPOSITION_TRANSLATION_DENOMINATOR_OFFSET_V3"
  | .reservedTail => "COMPOSITION_TRANSLATION_RESERVED_TAIL_OFFSET_V3"
end TranslationHeaderField

/-! ## Bounded topology and checked flattening model -/

structure SemanticEdge where
  child : Nat
  parent : Nat
  coefficient : Nat
  deriving DecidableEq, Repr

def EdgeStep (edges : List SemanticEdge) (child parent : Nat) : Prop :=
  ∃ edge ∈ edges, edge.child = child ∧ edge.parent = parent ∧ 0 < edge.coefficient

inductive Ancestor (edges : List SemanticEdge) : Nat → Nat → Prop where
  | direct (step : EdgeStep edges child parent) : Ancestor edges child parent
  | trans (leading : Ancestor edges child middle)
      (step : EdgeStep edges middle parent) : Ancestor edges child parent

def EdgesEarlier (edges : List SemanticEdge) : Prop :=
  ∀ edge ∈ edges, edge.child < edge.parent

theorem edge_step_strictly_increases
    (earlier : EdgesEarlier edges) (step : EdgeStep edges child parent) :
    child < parent := by
  rcases step with ⟨edge, member, rfl, rfl, _⟩
  exact earlier edge member

theorem ancestor_strictly_increases
    (earlier : EdgesEarlier edges) (path : Ancestor edges child parent) :
    child < parent := by
  induction path with
  | direct step => exact edge_step_strictly_increases earlier step
  | trans _ step induction =>
      exact Nat.lt_trans induction (edge_step_strictly_increases earlier step)

theorem earlier_edges_are_acyclic (earlier : EdgesEarlier edges) :
    ¬ Ancestor edges node node := by
  intro cycle
  exact (Nat.lt_irrefl node) (ancestor_strictly_increases earlier cycle)

inductive Reaches (edges : List SemanticEdge) : Nat → Nat → Prop where
  | refl : Reaches edges node node
  | step (edge : EdgeStep edges child parent)
      (tail : Reaches edges parent root) : Reaches edges child root

/-- A finite earlier-edge graph whose every non-root node has an observed
parent chain reaches its sole root.  This is the abstract form of the Rust
decoder's incoming-edge/root-index check; it does not execute the decoder. -/
theorem all_nodes_reach_root_of_parent_chain
    (earlier : EdgesEarlier edges)
    (hasParent : ∀ node, node < root →
      ∃ parent, EdgeStep edges node parent ∧ parent ≤ root) :
    ∀ node, node ≤ root → Reaches edges node root := by
  intro node bounded
  induction distance : root - node using Nat.strongRecOn generalizing node with
  | ind distance induction =>
      by_cases atRoot : node = root
      · subst node
        exact .refl
      · have belowRoot : node < root := Nat.lt_of_le_of_ne bounded atRoot
        obtain ⟨parent, step, parentBounded⟩ := hasParent node belowRoot
        have increasing : node < parent := edge_step_strictly_increases earlier step
        exact .step step (induction (root - parent) (by omega) parent parentBounded rfl)

/-- Exact finite topology assumptions admitted by the executable capacity
profile.  The unique sink is the final node, every earlier node has an
incoming parent edge, and the root cannot itself be a child. -/
structure SupportedTopology (edges : List SemanticEdge) where
  nodeCount : Nat
  root : Nat
  nonempty : 0 < nodeCount
  nodeCapacity : nodeCount ≤ maxNodes
  edgeCapacity : edges.length ≤ maxEdges
  rootIsLast : root + 1 = nodeCount
  earlier : EdgesEarlier edges
  hasParent : ∀ node, node < root →
    ∃ parent, EdgeStep edges node parent ∧ parent ≤ root
  rootHasNoParent : ∀ parent, ¬ EdgeStep edges root parent

theorem supported_topology_is_acyclic
    (topology : SupportedTopology edges) : ¬ Ancestor edges node node :=
  earlier_edges_are_acyclic topology.earlier

theorem supported_topology_reaches_unique_root
    (topology : SupportedTopology edges) (bounded : node < topology.nodeCount) :
    Reaches edges node topology.root := by
  apply all_nodes_reach_root_of_parent_chain topology.earlier topology.hasParent
  have rootIsLast := topology.rootIsLast
  omega

def witnessEdges : List SemanticEdge := [
  ⟨0, 2, 1⟩, ⟨1, 2, 2⟩, ⟨2, 3, 3⟩
]

theorem witness_edges_are_earlier : EdgesEarlier witnessEdges := by
  intro edge member
  simp [witnessEdges] at member
  rcases member with rfl | rfl | rfl <;> decide

theorem witness_all_nodes_reach_the_sole_root :
  ∀ node, node < 4 → Reaches witnessEdges node 3 := by
  intro node bounded
  have cases : node = 0 ∨ node = 1 ∨ node = 2 ∨ node = 3 := by omega
  rcases cases with rfl | rfl | rfl | rfl
  · exact .step ⟨⟨0, 2, 1⟩, by simp [witnessEdges], rfl, rfl, by decide⟩
      (.step ⟨⟨2, 3, 3⟩, by simp [witnessEdges], rfl, rfl, by decide⟩ .refl)
  · exact .step ⟨⟨1, 2, 2⟩, by simp [witnessEdges], rfl, rfl, by decide⟩
      (.step ⟨⟨2, 3, 3⟩, by simp [witnessEdges], rfl, rfl, by decide⟩ .refl)
  · exact .step ⟨⟨2, 3, 3⟩, by simp [witnessEdges], rfl, rfl, by decide⟩ .refl
  · exact .refl

structure Payoff where
  denominator : Nat
  numerators : List Nat
  deriving DecidableEq, Repr

structure WeightedPayoff where
  coefficient : Nat
  payoff : Payoff
  deriving DecidableEq, Repr

def numeratorAt (payoff : Payoff) (outcome : Nat) : Nat :=
  payoff.numerators[outcome]?.getD 0

def checkedMul128 (left right : Nat) : Option Nat :=
  let product := left * right
  if product ≤ maxU128 then some product else none

def checkedAdd128 (left right : Nat) : Option Nat :=
  let sum := left + right
  if sum ≤ maxU128 then some sum else none

def checkedWeightedNumerator (children : List WeightedPayoff)
    (commonDenominator outcome : Nat) : Option Nat := do
  let mut total := 0
  for child in children do
    if child.payoff.denominator = 0 ∨
        commonDenominator % child.payoff.denominator ≠ 0 then
      none
    let scaled ← checkedMul128 child.coefficient (numeratorAt child.payoff outcome)
    let contribution ← checkedMul128 scaled
      (commonDenominator / child.payoff.denominator)
    total ← checkedAdd128 total contribution
  pure total

structure CheckedFlattenRelation (outcomeCount : Nat)
    (children : List WeightedPayoff) (divisor common normalization : Nat)
    (output : Payoff) : Prop where
  positive : 0 < divisor ∧ 0 < common ∧ 0 < normalization ∧ 0 < output.denominator
  width : output.numerators.length = outcomeCount
  childWidths : ∀ child ∈ children, child.payoff.numerators.length = outcomeCount
  rawDenominatorChecked : common * divisor ≤ maxU128
  denominatorRelation : common * divisor = output.denominator * normalization
  numeratorRelation : ∀ outcome < outcomeCount,
    checkedWeightedNumerator children common outcome =
      some (numeratorAt output outcome * normalization)

theorem checked_flattening_equals_direct_composition
    (relation : CheckedFlattenRelation outcomeCount children divisor common normalization output)
    (bounded : outcome < outcomeCount)
    (direct : checkedWeightedNumerator children common outcome = some directNumerator) :
    numeratorAt output outcome * (common * divisor) =
      directNumerator * output.denominator := by
  have equalNumerator : directNumerator = numeratorAt output outcome * normalization := by
    have := relation.numeratorRelation outcome bounded
    rw [direct] at this
    exact Option.some.inj this
  rw [relation.denominatorRelation, equalNumerator]
  ac_rfl

def ExactMaterialization (quantity : Nat) (payoff : Payoff) (native : List Nat) : Prop :=
  native.length = payoff.numerators.length ∧
    ∀ outcome < payoff.numerators.length,
      native[outcome]?.getD 0 * payoff.denominator =
        quantity * numeratorAt payoff outcome

theorem checked_flattening_preserves_exact_materialization
    (relation : CheckedFlattenRelation outcomeCount children divisor common normalization output)
    (materialized : ExactMaterialization quantity output native)
    (bounded : outcome < outcomeCount)
    (direct : checkedWeightedNumerator children common outcome = some directNumerator) :
    native[outcome]?.getD 0 * (common * divisor) = quantity * directNumerator := by
  have outputBounded : outcome < output.numerators.length := by
    simpa [relation.width] using bounded
  have observed := materialized.2 outcome outputBounded
  have equalNumerator : directNumerator = numeratorAt output outcome * normalization := by
    have := relation.numeratorRelation outcome bounded
    rw [direct] at this
    exact Option.some.inj this
  rw [relation.denominatorRelation, equalNumerator]
  calc
    native[outcome]?.getD 0 * (output.denominator * normalization) =
        (native[outcome]?.getD 0 * output.denominator) * normalization := by ac_rfl
    _ = (quantity * numeratorAt output outcome) * normalization := by rw [observed]
    _ = quantity * (numeratorAt output outcome * normalization) := by ac_rfl

def nativeZero : Payoff := ⟨1, [1, 0, 0]⟩
def nativeTwo : Payoff := ⟨1, [0, 0, 1]⟩
def composed : Payoff := ⟨1, [1, 0, 2]⟩
def rootPayoff : Payoff := ⟨2, [3, 0, 6]⟩
def firstRecipe : List WeightedPayoff := [⟨1, nativeZero⟩, ⟨2, nativeTwo⟩]
def rootRecipe : List WeightedPayoff := [⟨3, composed⟩]

theorem first_recipe_checked_correspondence :
    CheckedFlattenRelation 3 firstRecipe 1 1 1 composed := by
  refine {
    positive := by decide
    width := by decide
    childWidths := ?_
    rawDenominatorChecked := by native_decide
    denominatorRelation := by decide
    numeratorRelation := ?_
  }
  · intro child member
    simp [firstRecipe] at member
    rcases member with rfl | rfl <;> decide
  · intro outcome bounded
    have cases : outcome = 0 ∨ outcome = 1 ∨ outcome = 2 := by omega
    rcases cases with rfl | rfl | rfl <;> native_decide

theorem root_recipe_checked_correspondence :
    CheckedFlattenRelation 3 rootRecipe 2 1 1 rootPayoff := by
  refine {
    positive := by decide
    width := by decide
    childWidths := ?_
    rawDenominatorChecked := by native_decide
    denominatorRelation := by decide
    numeratorRelation := ?_
  }
  · intro child member
    simp [rootRecipe] at member
    rcases member with rfl
    decide
  · intro outcome bounded
    have cases : outcome = 0 ∨ outcome = 1 ∨ outcome = 2 := by omega
    rcases cases with rfl | rfl | rfl <;> native_decide

theorem witness_exact_materialization :
    ExactMaterialization 2 rootPayoff [3, 0, 6] := by
  constructor
  · decide
  · intro outcome bounded
    have bounded' : outcome < 3 := by simpa [rootPayoff] using bounded
    have cases : outcome = 0 ∨ outcome = 1 ∨ outcome = 2 := by omega
    rcases cases with rfl | rfl | rfl <;> decide

theorem witness_supported_by_explicit_capacity_profile :
    2 ≤ 3 ∧ 3 ≤ maxOutcomes ∧ 4 ≤ maxNodes ∧
      witnessEdges.length ≤ maxEdges ∧ 6 ≤ maxTerms := by native_decide

/-! ## Canonical byte witness and hostile corpus -/

def zeros (count : Nat) : List UInt8 := List.replicate count 0
def repeated (value count : Nat) : List UInt8 := List.replicate count (UInt8.ofNat value)

def encodeDescriptorWitness : List UInt8 :=
  descriptorMagic ++ DClutch.Codec.encodeLE 2 schemaVersion ++ zeros 6 ++
  repeated 1 32 ++ repeated 2 32 ++ repeated 3 32 ++ repeated 4 32 ++
  repeated 40 32 ++ repeated 41 32 ++ repeated 30 32 ++
  repeated 50 32 ++ repeated 51 32 ++ capacityProfileId ++
  DClutch.Codec.encodeLE 4 3 ++ DClutch.Codec.encodeLE 4 4 ++
  DClutch.Codec.encodeLE 4 3 ++ DClutch.Codec.encodeLE 4 6 ++
  DClutch.Codec.encodeLE 8 2 ++ zeros 8

def encodeNode (id rank firstEdge edgeCount firstTerm termCount kind
    nativeOutcome divisor denominator : Nat) : List UInt8 :=
  repeated id 32 ++ DClutch.Codec.encodeLE 4 rank ++
  DClutch.Codec.encodeLE 4 firstEdge ++ DClutch.Codec.encodeLE 4 edgeCount ++
  DClutch.Codec.encodeLE 4 firstTerm ++ DClutch.Codec.encodeLE 4 termCount ++
  [UInt8.ofNat kind] ++ zeros 3 ++ DClutch.Codec.encodeLE 4 nativeOutcome ++ zeros 4 ++
  DClutch.Codec.encodeLE 8 divisor ++ DClutch.Codec.encodeLE 8 denominator

def encodeEdge (childId childIndex coefficient : Nat) : List UInt8 :=
  repeated childId 32 ++ DClutch.Codec.encodeLE 4 childIndex ++ zeros 4 ++
  DClutch.Codec.encodeLE 8 coefficient

def encodeTerm (outcome numerator : Nat) : List UInt8 :=
  DClutch.Codec.encodeLE 4 outcome ++ zeros 4 ++ DClutch.Codec.encodeLE 8 numerator

def graphNodesWitness : List UInt8 :=
  encodeNode 10 0 0 0 0 1 0 0 1 1 ++
  encodeNode 11 0 0 0 1 1 0 2 1 1 ++
  encodeNode 20 1 0 2 2 2 1 0 1 1 ++
  encodeNode 30 2 2 1 4 2 1 0 2 2

def graphEdgesWitness : List UInt8 :=
  encodeEdge 10 0 1 ++ encodeEdge 11 1 2 ++ encodeEdge 20 2 3

def graphTermsWitness : List UInt8 :=
  encodeTerm 0 1 ++ encodeTerm 2 1 ++ encodeTerm 0 1 ++
  encodeTerm 2 2 ++ encodeTerm 0 3 ++ encodeTerm 2 6

def encodeGraphWitness : List UInt8 :=
  graphMagic ++ DClutch.Codec.encodeLE 2 schemaVersion ++ zeros 6 ++
  repeated 40 32 ++ repeated 30 32 ++
  DClutch.Codec.encodeLE 4 3 ++ DClutch.Codec.encodeLE 4 4 ++
  DClutch.Codec.encodeLE 4 3 ++ DClutch.Codec.encodeLE 4 6 ++
  DClutch.Codec.encodeLE 4 3 ++ zeros 12 ++ graphNodesWitness ++
  graphEdgesWitness ++ graphTermsWitness

def rootTermBytesWitness : List UInt8 := encodeTerm 0 3 ++ encodeTerm 2 6

def encodeTranslationWitness : List UInt8 :=
  translationMagic ++ DClutch.Codec.encodeLE 2 schemaVersion ++ zeros 6 ++
  repeated 40 32 ++ repeated 30 32 ++
  DClutch.Codec.encodeLE 4 3 ++ DClutch.Codec.encodeLE 4 2 ++
  DClutch.Codec.encodeLE 8 2 ++ zeros 32 ++ rootTermBytesWitness

def patch (bytes : List UInt8) (offset : Nat) (replacement : List UInt8) : List UInt8 :=
  bytes.take offset ++ replacement ++ bytes.drop (offset + replacement.length)

def hostileDescriptorReserved : List UInt8 := List.set encodeDescriptorWitness 10 1
def hostileGraphCycle : List UInt8 :=
  patch (patch encodeGraphWitness 528 (repeated 30 32)) 560 (DClutch.Codec.encodeLE 4 3)
def hostileGraphDuplicateNode : List UInt8 :=
  patch encodeGraphWitness 192 (repeated 10 32)
def hostileGraphAmbiguousRoot : List UInt8 :=
  patch encodeGraphWitness 48 (repeated 20 32)
def hostileTranslationMismatch : List UInt8 :=
  List.set encodeTranslationWitness 152 7
def hostileTranslationReserved : List UInt8 :=
  List.set encodeTranslationWitness 96 1

theorem fixed_widths_are_exact :
    descriptorBytes = 368 ∧ graphHeaderBytes = 112 ∧ nodeBytes = 80 ∧
      edgeBytes = 48 ∧ termBytes = 16 ∧ translationHeaderBytes = 128 := by
  native_decide

theorem layouts_are_pairwise_disjoint :
    descriptorLayout.Pairwise Before ∧ graphHeaderLayout.Pairwise Before ∧
      nodeLayout.Pairwise Before ∧ edgeLayout.Pairwise Before ∧
      termLayout.Pairwise Before ∧ translationHeaderLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 descriptorSchema,
    specializeFrom_pairwise 0 graphHeaderSchema,
    specializeFrom_pairwise 0 nodeSchema,
    specializeFrom_pairwise 0 edgeSchema,
    specializeFrom_pairwise 0 termSchema,
    specializeFrom_pairwise 0 translationHeaderSchema⟩

theorem witness_byte_widths_are_exact :
    encodeDescriptorWitness.length = 368 ∧ encodeGraphWitness.length = 672 ∧
      encodeTranslationWitness.length = 160 := by native_decide

theorem root_translation_is_byte_identical_to_canonical_graph_root :
    encodeTranslationWitness.drop translationHeaderBytes = rootTermBytesWitness ∧
      encodeGraphWitness.drop (graphHeaderBytes + 4 * nodeBytes + 3 * edgeBytes + 4 * termBytes) =
        rootTermBytesWitness := by native_decide

theorem schema_and_capacity_coordinates_have_exact_width :
    capacityProfileId.length = 32 ∧ descriptorSchemaId.length = 32 ∧
      graphSchemaId.length = 32 ∧ translationSchemaId.length = 32 := by native_decide

theorem hostile_corpus_preserves_same_width_where_intended :
    hostileDescriptorReserved.length = encodeDescriptorWitness.length ∧
      hostileGraphCycle.length = encodeGraphWitness.length ∧
      hostileGraphDuplicateNode.length = encodeGraphWitness.length ∧
      hostileGraphAmbiguousRoot.length = encodeGraphWitness.length ∧
      hostileTranslationMismatch.length = encodeTranslationWitness.length ∧
      hostileTranslationReserved.length = encodeTranslationWitness.length := by native_decide

end DClutch.RepresentationCompositionV3Abi
