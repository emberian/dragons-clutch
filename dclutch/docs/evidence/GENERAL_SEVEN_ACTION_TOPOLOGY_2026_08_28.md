# The caller rung under GEN-SEVEN — and the two actions the old catalogue omitted

Lane GEN-SEVEN, 2026-08-28. This is design and executable contract evidence, not
release evidence. No new General artifact triple, caller ELF, release, frame or
deployment is admitted by this lane.

## 1. Finding: the 14-action release must remain refused

The seven collection/candidate transitions already exist as pure contracts, but
`ControllerRequestV2` cannot legally request any one of them. Its canonical
validator refuses every non-settlement action. More sharply, its final two bump
bytes cannot name the three independently derived states in one terminal
`VerifyCandidateRow`: Candidate, verifier cursor, and immutable verified-candidate
result.

Adding those seven tags to the old decoder would still leave two unsound holes:

1. `Consider` currently names only the best-candidate selection state. The pure
   candidate owner also requires `GeneralCandidateV1::record_considered`, and the
   work escrow must pay the final consideration crank in the same transition.
2. `GeneralCandidateV1::close_out` and `WorkEscrowClosePlanV1` have no controller
   action. A host-side cleanup after an unrelated action would have no admitted
   AccountProfile, TransitionProgram or EffectProgram of its own.

Therefore the old 14-action `Complete` release remains refused. The fix is a
versioned action and request topology, followed by one coordinated artifact
regeneration. It is not a permissive interpretation of V2.

## 2. Frozen request rung

`ControllerRequestV3` is an exact 64-byte wire break. It preserves the packet
width and the settlement selector/cursor offsets while assigning all three tail
bytes as action-selected state bump witnesses.

| bytes | field | canonical rule |
|---:|---|---|
| 0..7 | magic | `DCGREQ03` |
| 8..9 | version | little-endian `3` |
| 10 | action | tags 0..13 preserve V2; tag 14 is `CloseCandidate` |
| 11 | manifest order | nonzero only for `Collect` and `Distribute` |
| 12..15 | reserved | all zero |
| 16..23 | expected revision | action-selected; zero where no revision exists |
| 24..55 | subject identity | nonzero for every action except `Freeze` |
| 56..59 | page index | action-selected |
| 60 | execution index | action-selected |
| 61 | primary state bump | always the first state coordinate, including bump 0 |
| 62 | secondary state bump | second state coordinate, or zero when unused |
| 63 | result state bump | only terminal verification may use it |

The layout is authored in Lean, generated into Rust, and freshness-tested byte
for byte. The Rust decoder additionally enforces one exact field grammar for
each of the fifteen actions. Zero is a valid PDA bump; presence is determined by
the action topology, never by whether a bump byte is zero.

## 3. Frozen action topology

The executable front-half planner returns the following state, signer,
environment, custody, Claims and lamport obligations. It does not execute an
artifact or claim those artifacts exist.

| action | mutable states | signer / trusted input | physical obligations |
|---|---|---|---|
| `Consider` | Selection authenticate-or-create; Candidate authenticate | permissionless | Candidate records consideration; final consideration crank is paid from its verification compartment |
| `OpenBatch` | Batch create; General root tail writable | permissionless; `CurrentSlot` | no Custody or Claims move |
| `PlaceOrder` | Batch authenticate; Order create | order owner; `CurrentSlot` | external collateral enters the order's Settlement vault; Claims Position is created |
| `CancelOrder` | Batch and Order authenticate | order owner; `CurrentSlot` | order Settlement collateral returns to owner; Claims Position closes/refunds |
| `CloseBatch` | Batch authenticate; General root tail writable | permissionless; `CurrentSlot` | no Custody or Claims move |
| `SubmitCandidate` | Candidate create | solver; `CurrentSlot` | exact verification and cleanup work escrow plus rent is funded |
| `VerifyCandidateRow` | Candidate authenticate; verifier authenticate-or-create; verified result create only on the terminal row | permissionless | one verification crank is paid from Candidate work escrow |
| `ReleaseOrder` | Order authenticate | permissionless; `CurrentSlot` | residual order Settlement collateral returns to owner; Claims Position closes/refunds |
| `CloseCandidate` | Candidate close | permissionless; `CurrentSlot` | cleanup crank goes to the caller; unused verification work and Candidate rent return to the solver; Candidate ends at zero lamports |

`CloseCandidate` is allowed only when the Candidate is already `Considered`, or
when the joined Batch's settlement-close slot has passed. That makes cleanup
permissionless without letting an arbitrary caller censor a live submitted or
partially verified candidate. The planner requires the Candidate's embedded
batch identity to equal the authenticated Batch identity before constructing
the real `WorkEscrowClosePlanV1`.

The existing custody table remains the sole authority for collateral direction.
This rung only says which actions require a Claims Position lifecycle; that
lifecycle is still missing and cannot be inferred from the custody transfer.

## 4. Release-profile consequence

`Consider` keeps tag 0 but its triple must change because it gains Candidate
state authentication/mutation and a lamport effect. `CloseCandidate` adds tag
14. Thus the next complete General catalogue contains fifteen actions, not
fourteen.

The old profile counts must not be silently reinterpreted. A versioned release
profile should have these exact sets:

| proposed versioned profile | entries | contents |
|---|---:|---|
| `SettlementOnly` | 7 | the seven settlement action triples |
| `SettlementWithActivation` | 8 | settlement plus the activation descriptor |
| `CompleteV2` | 15 | all fifteen action triples |
| `CompleteV2WithActivation` | 16 | all fifteen triples plus activation |

The existing 14/15-entry `Complete` profiles remain the old catalogue. A new
release-schema/profile discriminator must make the 15-entry collision
unambiguous: old complete-plus-activation is not new complete-without-activation.
All fifteen action triples, including regenerated `Consider`, are one batched
release unit.

## 5. Executable integration order

1. Land the versioned action catalogue, V3 request schema and exact RequestProfile
   rows for all fifteen actions while keeping release admission closed.
2. Grow the General register bank once: the new state identities and scalar
   fields, the item stride, and trusted `CurrentSlot` coordinates must be one
   generated numbering change.
3. Teach StateLifecycle to consume the multi-state topology, including
   Candidate mutation during `Consider` and Candidate closure during
   `CloseCandidate`.
4. Author all fifteen AccountProfiles, TransitionPrograms and EffectPrograms as
   one joined set. This includes physical work-escrow lamport coordinates and
   the missing Claims Position create/close/refund lifecycle.
5. Extend the accelerator evaluator and operator derivation so each request has
   a real caller rather than only an artifact builder.
6. Add the versioned 7/8/15/16 release admission and require every descriptor,
   artifact identity, program-set coordinate and schema digest to join.
7. Run the complete caller campaign:
   open → place/cancel → close batch → submit → verify → consider → candidate
   close → release/settlement, including resume and adversarial substitutions.
8. Build the real SBF links, use the ALT/v0 route at canonical width N=258, run
   the frame-diagnostic script on every link, and report compute as pass count
   plus the 20-seed mean.

Dependencies that remain outside this isolated rung are the shared register and
artifact regeneration, the versioned release manifest/program-set admission,
the Claims Position lifecycle, physical lamport-effect coordinates, and the
caller/campaign integration. This lane deliberately does not edit the active V7
or Founding seams.

## 6. What was and was not measured

The codec and topology contracts have focused adversarial tests for action
substitution, unused-coordinate smuggling, zero-subject substitution,
settlement/front-half confusion, zero-valued-but-present PDA bumps, cross-batch
Candidate close, and premature permissionless close.

No SBF link or caller artifact was produced, so there is no honest account-lock,
packet-frame or CU census row yet. Those measurements belong after step 7 has a
real caller and step 8 has the exact link that would ship.
