# The item `OUTCOME` register — a proposal, and why it is not landed alone

**Status:** design note. Nothing here is implemented. The ordering decision
belongs to the channel lane; see "Why the order matters".

## The register

General's per-outcome item slot is six scalars wide
(`GENERAL_HOT_ITEM_SCALAR_STRIDE_V3 = 6`,
`crates/dclutch-general-adapter-contract/src/hot_candidate_v3.rs:66`), and
coordinate 0 of that slot is `item_scalar::OUTCOME` — "canonical Product outcome
index" (`hot_candidate_v3.rs:430`).

**Every writer writes the item's own index, and the reader refuses anything
else.** Thirteen actions declare the six-wide stride; `OpenBatch` and
`CloseBatch` declare zero since `ea4c46e0`. The writers, in
`crates/dclutch-general-adapter-contract/src/hot_candidate_v3.rs`:

| line | owner | form |
|---|---|---|
| 1381 | `project_general_place_order_candidate_in_place_v3` | `(item_scalar::OUTCOME, u64::from(item))` |
| 1624 | `project_general_cancel_order_candidate_in_place_v3` | same |
| 1840 | `project_general_release_order_candidate_in_place_v3` | same |
| 2441 | `project_general_verify_candidate_summary_into_bank_v3` | same |
| 4268, 4498, 4670 | the host bank builders | `write_scalar(.., base + item_scalar::OUTCOME, u64::from(item))` |

and the readers, which refuse any other value:

| line | owner |
|---|---|
| 1014 | `project_general_submit_candidate_in_place_v3` — `!= u64::from(item)` ⇒ `InvalidCoordinate` |
| 2878 | `apply_general_initialize_candidate_v3` |
| 3259 | `apply_general_hot_candidate_v3` |
| 5057, 5903 | the last-item assertions |

So the register is an identity function of the coordinate it occupies. That is
exactly the observation `ea4c46e0` acted on for the two batch actions, whose only
item instruction was the shared bound check over it.

## What it costs

Measured 2026-09-02 on the real-ELF General `OpenBatch` campaign, perturbed back
to the six-wide stride so it reads the geometry the other thirteen actions have
(the perturbation is `git revert -n ea4c46e0`, measurement only, never
committed), at HEAD `28ff0823`:

| geometry | slope | honest maximum committed width |
|---|---:|---:|
| before tonight's derivation-bank change | 528 B/outcome | N = 13 |
| after it | 384 B/outcome | N = 30 |

384 B/outcome is eight full-width scalar banks live at the peak, each costing
`8 × stride` bytes per outcome. **Stride 6 → 5 is therefore 64 B/outcome, one
sixth of the remaining slope: 384 → 320.** Extrapolating the measured five-page
tier puts the maximum committed width somewhere near N = 36; only a measurement
settles it, because the transport page count steps with the bank width and each
step costs a scratch-page account and a chunk authority.

## Two consumers, and only one of them is vacuous

**The bound check is vacuous.** `transition_artifacts_v3.rs:1289` emits the
shared item instruction `scalar_lt(item_scalar::OUTCOME, scalar::OUTCOME_COUNT)`
— a bound check on a register whose sole legal value is the coordinate it
occupies, over a bank whose width is `OUTCOME_COUNT` by construction.

**The Claims row field is not.** `effect_artifacts_v3.rs:2721` maps
`item_scalar::OUTCOME` onto `AffineBatchRequestLayoutV2::ROW_OUTCOME` — four
bytes of the repeated row template in the child Claims affine-batch request
(`crates/dclutch-claims-svm/src/affine_batch_v2.rs:109`). The effect's
item-coordinate list at `effect_artifacts_v3.rs:2767` names it alongside the
three magnitudes. **Deleting the register deletes the only thing the effect can
copy into that field**, so the ABI change is not a deletion; it is a question
put to the Claims wire:

- **(a) Claims reads the outcome positionally.** Drop `ROW_OUTCOME` and let row
  `i` mean outcome `i`. This is a semantic claim about the row table — dense,
  ordered, exactly `OUTCOME_COUNT` long — that has to be proved where Claims
  authenticates the batch, not asserted here. It is the cheaper end state and it
  removes a field from the child wire as well as a register from the bank.
- **(b) The effect kernel gains an item-index operand.** A `write_request_u32`
  whose source is the item's own coordinate rather than a register. This keeps
  the Claims wire byte-identical and moves the identity function from a bank into
  the projector. It is the smaller blast radius and the larger permanent surface.

Neither is a General decision alone: (a) changes a Claims request, (b) changes
the EffectV4 instruction set that every family shares.

## What the change costs to make

The stride is a four-artifact join and Lean is its author. `ea4c46e0` records
the route: `DClutchSemantics.GeneralTransitionV3.actionItemScalarStride` and
`GeneralRequestProfilesV1.actionItemScalarStride` move first, the generated
transition programs and request profiles are re-emitted, and the byte gate
`every_authored_program_is_byte_identical_to_the_lean_authored_one` is what makes
that the only admissible order. `artifacts_v3.rs:614-636` then joins account,
request, transition and effect to `general_hot_item_scalar_stride_v3`, and any
subset moving alone refuses at admission.

Renumbering is mechanical but total: `QUANTITY` 1→0,
`CLAIMS_AGGREGATE_MAGNITUDE` 2→1, `CLAIMS_SOURCE_MAGNITUDE` 3→2,
`CLAIMS_DESTINATION_MAGNITUDE` 4→3, `CURSOR_INVENTORY` 5→4. Every General
artifact digest moves, as it did for the 151-scalar widening; General has no
published on-chain substrate, so the re-digest strands nothing.

## What it does to the Dealer equity bank

Directly, nothing. `dealer_equity_scalar_count_v3(action)` takes no outcome count
(`crates/dclutch-operator/src/dealer_equity_hot_v3.rs:167`) and is required to
equal `profile.common_scalar_count()` (`:385`) — that bank has no item tail and
is flat in the Product width. A General stride change cannot move it.

What couples them is the transport, not the bank.
`classify_bank_transport_v2(scalars, identities)` derives the page count from
total bank bytes — `ceil((8·scalars + 32·identities) / 880)` — so a narrower
General item slot moves General's page tiers and therefore its chunk-authority
span and its account frame. Measured tonight, the four-page tier ends at N = 18
at stride 6 and would end at N = 21 at stride 5. **Any sizing the channel lane
does against General's current page tiers is sizing against a number that is
still in motion**, and the honest thing is to size against the formula, not
against 4.

## Why the order matters, and why this is not landed alone

The 384 B/outcome that remains after tonight's change decomposes, measured by
phase mark, as:

| phase | B/outcome | banks |
|---|---:|---:|
| `p5r-projection-banks` — the request/account projection's three pairs | 144 | 3 |
| `p5-sealed-ownership-arena` — the preplan output pair | 48 | 1 |
| `candidate` — the admitted-candidate chunked CPI | 192 | 4 |

**The channel lane owns the 192.** If the output-page transport removes those
four banks, the slope falls to 192 with no ABI change at all, and this proposal
is then worth 32 B/outcome rather than 64 — a third of what it is worth today.
Landing an ABI change that moves every General artifact digest, to buy a
saving whose size depends on a lane that is mid-flight, is the wrong order. The
channel lane's outcome decides whether this is worth its blast radius, and this
note exists so that decision is made with the number in front of it rather than
rediscovered.

## The one thing worth doing now regardless

The bound check at `transition_artifacts_v3.rs:1289` is vacuous for all thirteen
remaining actions for exactly the reason it was vacuous for the two batch ones —
`hot_candidate_v3` already refuses any value but the coordinate. Removing the
instruction (not the register) is a transition-program change with no ABI
consequence: it costs one item instruction per action and buys the CU of
executing it `N` times. It does not need the Claims decision above.
