/-!
# Exact rational claim representations

This module separates three facts which the first representation prototype
compressed into one exact-lot restriction:

* Claims owns native/materialized claim conservation;
* a token adapter owns shard and receipt Mint supply and holder balances; and
* immutable representation records own the recipe which relates them.

For one outcome, `D * C = F` is the denomination invariant: `C` native claims
are held by the canonical shard custody Position and `F` transferable shard
atoms exist.  A Structured receipt with supply `S` and coefficient `c` holds
`S * c` of those shard atoms.  The remaining `R` shard atoms are ordinary
Token-owned transferable balances, so the joined accounting equation is

`D * C = S * c + R`.

`R` is never an account-local rounding credit.  It can be transferred and
coalesced with other shard balances until an exact multiple of `D` can
reconstitute native claims.

The final section models immutable representation recipes as a
content-addressed DAG.  Every node carries a checked, common-scale native
exposure certificate.  Edges strictly decrease rank, child identities are
canonical, and the root is therefore flattened before runtime claim mutation.
The physical record adapter remains responsible for authenticating each
content digest against the exact canonical record bytes.
-/

namespace DClutch.RationalRepresentationV2

/-! ## Exact denomination and explicit change -/

/-- Token and Claims observations for one denominated native outcome. -/
structure ShardCoordinate where
  nativeLocked : Nat
  shardSupply : Nat
  deriving DecidableEq, Repr

/-- Every live shard atom has exact native backing while the Market is open. -/
def ShardCoordinate.exact (denominator : Nat) (coordinate : ShardCoordinate) : Prop :=
  0 < denominator ∧ coordinate.shardSupply = denominator * coordinate.nativeLocked

/-- Convert native claims into exact shard atoms. -/
def denominate
    (denominator quantity : Nat) (coordinate : ShardCoordinate) : ShardCoordinate := {
  nativeLocked := coordinate.nativeLocked + quantity
  shardSupply := coordinate.shardSupply + denominator * quantity
}

/-- Burn an exact multiple of shard atoms and release native claims. -/
def reconstitute
    (denominator quantity : Nat) (coordinate : ShardCoordinate) : ShardCoordinate := {
  nativeLocked := coordinate.nativeLocked - quantity
  shardSupply := coordinate.shardSupply - denominator * quantity
}

theorem denominate_preserves_exact
    (denominator quantity : Nat) (coordinate : ShardCoordinate)
    (exact : coordinate.exact denominator) :
    (denominate denominator quantity coordinate).exact denominator := by
  constructor
  · exact exact.1
  · simp [denominate, exact.2, Nat.mul_add]

theorem reconstitute_preserves_exact
    (denominator quantity : Nat) (coordinate : ShardCoordinate)
    (exact : coordinate.exact denominator) :
    (reconstitute denominator quantity coordinate).exact denominator := by
  constructor
  · exact exact.1
  · simp only [reconstitute]
    rw [exact.2, Nat.mul_sub_left_distrib]

/-- One coordinate of a Structured receipt projection.  All four values are
authenticated observations of Claims or Token-owned state, not a second
persisted representation ledger. -/
structure StructuredCoordinate where
  coefficient : Nat
  nativeLocked : Nat
  shardSupply : Nat
  structuredCustody : Nat
  explicitFreeShards : Nat
  deriving DecidableEq, Repr

/-- The full join between denomination and Structured custody. -/
def StructuredCoordinate.exact
    (denominator receiptSupply : Nat) (coordinate : StructuredCoordinate) : Prop :=
  0 < denominator ∧
  coordinate.shardSupply = denominator * coordinate.nativeLocked ∧
  coordinate.structuredCustody = receiptSupply * coordinate.coefficient ∧
  coordinate.shardSupply =
    coordinate.structuredCustody + coordinate.explicitFreeShards

/-- Equivalent expanded form of the Structured conservation equation. -/
theorem exact_implies_explicit_remainder_equation
    (denominator receiptSupply : Nat) (coordinate : StructuredCoordinate)
    (exact : coordinate.exact denominator receiptSupply) :
    denominator * coordinate.nativeLocked =
      receiptSupply * coordinate.coefficient + coordinate.explicitFreeShards := by
  rw [← exact.2.1, exact.2.2.2, exact.2.2.1]

/-- Issuing receipts moves exact shard atoms from Token-owned free balances into
Structured custody.  It creates neither native claims nor shard atoms. -/
def issueReceipts
    (quantity : Nat) (coordinate : StructuredCoordinate) : StructuredCoordinate := {
  coordinate with
  structuredCustody := coordinate.structuredCustody + quantity * coordinate.coefficient
  explicitFreeShards := coordinate.explicitFreeShards - quantity * coordinate.coefficient
}

/-- Unwrapping receipts releases their exact shard backing as ordinary
transferable shard atoms. -/
def unwrapReceipts
    (quantity : Nat) (coordinate : StructuredCoordinate) : StructuredCoordinate := {
  coordinate with
  structuredCustody := coordinate.structuredCustody - quantity * coordinate.coefficient
  explicitFreeShards := coordinate.explicitFreeShards + quantity * coordinate.coefficient
}

theorem issue_receipts_preserves_exact
    (denominator receiptSupply quantity : Nat) (coordinate : StructuredCoordinate)
    (exact : coordinate.exact denominator receiptSupply)
    (available : quantity * coordinate.coefficient ≤ coordinate.explicitFreeShards) :
    (issueReceipts quantity coordinate).exact denominator (receiptSupply + quantity) := by
  rcases exact with ⟨positive, denomination, custody, partition⟩
  refine ⟨positive, denomination, ?_, ?_⟩
  · simp [issueReceipts, custody, Nat.add_mul]
  · simp only [issueReceipts]
    rw [partition]
    omega

theorem unwrap_receipts_preserves_exact
    (denominator receiptSupply quantity : Nat) (coordinate : StructuredCoordinate)
    (exact : coordinate.exact denominator receiptSupply)
    (available : quantity ≤ receiptSupply) :
    (unwrapReceipts quantity coordinate).exact denominator (receiptSupply - quantity) := by
  rcases exact with ⟨positive, denomination, custody, partition⟩
  refine ⟨positive, denomination, ?_, ?_⟩
  · simp only [unwrapReceipts]
    rw [custody, Nat.sub_mul]
  · simp only [unwrapReceipts]
    rw [partition, custody]
    have backingAvailable :
        quantity * coordinate.coefficient ≤ receiptSupply * coordinate.coefficient :=
      Nat.mul_le_mul_right coordinate.coefficient available
    omega

/-- Coalesce explicit free shard atoms from any set of Token holders.  The
change remains an ordinary transferable shard balance. -/
structure Coalescing where
  inputShards : Nat
  nativeClaims : Nat
  changeShards : Nat
  deriving DecidableEq, Repr

def coalesce (denominator inputShards : Nat) : Option Coalescing :=
  if 0 < denominator then
    some {
      inputShards
      nativeClaims := inputShards / denominator
      changeShards := inputShards % denominator
    }
  else none

theorem coalescing_is_exact
    (denominator inputShards : Nat) (positive : 0 < denominator) :
    match coalesce denominator inputShards with
    | some result =>
        result.inputShards = denominator * result.nativeClaims + result.changeShards ∧
        result.changeShards < denominator
    | none => False := by
  simp [coalesce, positive, Nat.mod_lt, Nat.div_add_mod]

theorem subdenominator_change_has_no_hidden_payout
    (denominator inputShards : Nat) (positive : 0 < denominator)
    (small : inputShards < denominator) :
    coalesce denominator inputShards = some {
      inputShards
      nativeClaims := 0
      changeShards := inputShards
    } := by
  simp [coalesce, positive, Nat.div_eq_of_lt small, Nat.mod_eq_of_lt small]

/-! ## Canonically flattened representation DAG -/

/-- A common-scale native exposure vector. -/
structure Exposure where
  quantities : List Nat
  deriving DecidableEq, Repr

def Exposure.widthExact (outcomeCount : Nat) (exposure : Exposure) : Bool :=
  exposure.quantities.length = outcomeCount

def scaleVector (factor : Nat) (values : List Nat) : List Nat :=
  values.map fun value => factor * value

def addVectors (left right : List Nat) : List Nat :=
  List.zipWith (.+.) left right

def zeroVector (width : Nat) : List Nat :=
  List.replicate width 0

def oneHot (width outcome scale : Nat) : List Nat :=
  (List.range width).map fun index => if index = outcome then scale else 0

inductive NodeKind where
  | native (outcome : Nat)
  | shard (denominator : Nat)
  | basket
  deriving DecidableEq, Repr

/-- An edge repeats an exact number of child atoms per parent atom.  The child
projection is repeated in the immutable parent record so it can be joined to
the finalized child record without recursive runtime authority. -/
structure Edge where
  childId : Nat
  childRank : Nat
  atomsPerParent : Nat
  childExposure : Exposure
  deriving DecidableEq, Repr

structure Node where
  contentId : Nat
  rank : Nat
  kind : NodeKind
  edges : List Edge
  exposure : Exposure
  deriving DecidableEq, Repr

structure Graph where
  graphId : Nat
  outcomeCount : Nat
  scale : Nat
  rootId : Nat
  nodes : List Node
  deriving DecidableEq, Repr

def strictlyIncreasing : List Nat → Bool
  | [] | [_] => true
  | left :: right :: rest => left < right && strictlyIncreasing (right :: rest)

def edgeContribution (edge : Edge) : List Nat :=
  scaleVector edge.atomsPerParent edge.childExposure.quantities

def sumEdges (width : Nat) (edges : List Edge) : List Nat :=
  edges.foldl (fun total edge => addVectors total (edgeContribution edge)) (zeroVector width)

def maxChildRank (edges : List Edge) : Nat :=
  edges.foldl (fun rank edge => max rank edge.childRank) 0

def edgeAuthenticated (nodes : List Node) (edge : Edge) : Bool :=
  nodes.any fun node =>
    node.contentId = edge.childId && node.rank = edge.childRank &&
    node.exposure = edge.childExposure

def edgesCommonValid
    (outcomeCount parentRank : Nat) (nodes : List Node) (edges : List Edge) : Bool :=
  edges.all fun edge =>
    edge.childId != 0 && edge.atomsPerParent != 0 &&
    edge.childRank < parentRank && edge.childExposure.widthExact outcomeCount &&
    edgeAuthenticated nodes edge

def Node.localValid
    (outcomeCount scale : Nat) (nodes : List Node) (node : Node) : Bool :=
  node.contentId != 0 && node.exposure.widthExact outcomeCount &&
  edgesCommonValid outcomeCount node.rank nodes node.edges &&
  strictlyIncreasing (node.edges.map fun edge => edge.childId) &&
  match node.kind with
  | .native outcome =>
      node.rank = 0 && node.edges.isEmpty && outcome < outcomeCount &&
      node.exposure.quantities = oneHot outcomeCount outcome scale
  | .shard denominator =>
      1 < denominator && node.edges.length = 1 &&
      node.edges.all fun edge =>
        edge.atomsPerParent = 1 && node.rank = edge.childRank + 1 &&
        scaleVector denominator node.exposure.quantities = edge.childExposure.quantities
  | .basket =>
      !node.edges.isEmpty && node.rank = maxChildRank node.edges + 1 &&
      node.exposure.quantities = sumEdges outcomeCount node.edges

def Graph.valid (graph : Graph) : Bool :=
  graph.graphId != 0 && 0 < graph.outcomeCount && 0 < graph.scale &&
  graph.rootId != 0 && !graph.nodes.isEmpty &&
  decide (graph.nodes.map fun node => node.contentId).Nodup &&
  (graph.nodes.all fun node => node.localValid graph.outcomeCount graph.scale graph.nodes) &&
  graph.nodes.any fun node => node.contentId = graph.rootId

theorem valid_graph_has_selected_root
    (graph : Graph) (valid : graph.valid = true) :
    graph.nodes.any (fun node => node.contentId = graph.rootId) = true := by
  simp only [Graph.valid, Bool.and_eq_true] at valid
  exact valid.2

theorem common_valid_edges_strictly_decrease
    (outcomeCount parentRank : Nat) (nodes : List Node) (edges : List Edge)
    (valid : edgesCommonValid outcomeCount parentRank nodes edges = true) :
    edges.all (fun edge => edge.childRank < parentRank) = true := by
  simp only [edgesCommonValid, List.all_eq_true, Bool.and_eq_true] at valid ⊢
  intro edge member
  exact (valid edge member).1.1.2

theorem valid_node_edges_strictly_decrease
    (outcomeCount scale : Nat) (nodes : List Node) (node : Node)
    (valid : node.localValid outcomeCount scale nodes = true) :
    node.edges.all (fun edge => edge.childRank < node.rank) = true := by
  simp only [Node.localValid, Bool.and_eq_true] at valid
  exact common_valid_edges_strictly_decrease
    outcomeCount node.rank nodes node.edges valid.1.1.2

/-- The canonical native exposure of the selected root.  It is a borrowed
projection in the physical kernel; no runtime wrapper graph owns liabilities. -/
def Graph.rootExposure? (graph : Graph) : Option Exposure :=
  (graph.nodes.find? fun node => node.contentId = graph.rootId).map fun node => node.exposure

theorem root_exposure_is_precomputed
    (graph : Graph) (node : Node)
    (found : graph.nodes.find? (fun candidate => candidate.contentId = graph.rootId) = some node) :
    graph.rootExposure? = some node.exposure := by
  simp [Graph.rootExposure?, found]

/-! ## Immutable descriptor authority -/

/-- Immutable representation authority. Mutable Claims quantities, Token
supplies and holder balances, and replay revisions are deliberately absent. -/
structure ImmutableDescriptor where
  descriptorId : Nat
  graphId : Nat
  rootId : Nat
  marketId : Nat
  releaseSetId : Nat
  receiptMint : Nat
  tokenProgram : Nat
  representationAuthority : Nat
  denominator : Nat
  coefficients : List Nat
  deriving DecidableEq, Repr

/-- The descriptor selects one exact graph and root, rather than merely an
arbitrary same-width exposure. -/
def ImmutableDescriptor.bindsGraph
    (descriptor : ImmutableDescriptor) (graph : Graph) : Bool :=
  descriptor.graphId = graph.graphId && descriptor.rootId = graph.rootId

theorem bindsGraph_iff_exact_identities
    (descriptor : ImmutableDescriptor) (graph : Graph) :
    descriptor.bindsGraph graph = true ↔
      descriptor.graphId = graph.graphId ∧ descriptor.rootId = graph.rootId := by
  simp [ImmutableDescriptor.bindsGraph]

/-- Every coefficient is the same exact common-scale payoff as the selected
graph root: `c_i * graphScale = rootExposure_i * denominator`. -/
def coefficientPayoffsExact
    (denominator graphScale : Nat) (coefficients exposure : List Nat) : Bool :=
  coefficients.length = exposure.length &&
    (List.zipWith
      (fun coefficient native => coefficient * graphScale == native * denominator)
      coefficients exposure).all id

/-- Full immutable descriptor→graph admission. Record finality and content
hashing remain separately named physical-adapter assumptions. -/
def ImmutableDescriptor.validFor
    (descriptor : ImmutableDescriptor) (graph : Graph) : Bool :=
  descriptor.descriptorId != 0 && descriptor.marketId != 0 &&
  descriptor.releaseSetId != 0 && descriptor.receiptMint != 0 &&
  descriptor.tokenProgram != 0 && descriptor.representationAuthority != 0 &&
  0 < descriptor.denominator && descriptor.bindsGraph graph &&
  descriptor.coefficients.length = graph.outcomeCount &&
  match graph.rootExposure? with
  | some exposure => coefficientPayoffsExact
      descriptor.denominator graph.scale descriptor.coefficients exposure.quantities
  | none => false

end DClutch.RationalRepresentationV2
