# Dealer runtime semantic schema V1

These are local semantic-body coordinates, not global Solana account tags.
Every body begins with an 8-byte magic, little-endian `u16` version `1`, and a
zero `u16` flags word. Integers are little-endian. Every omitted fixed-width
entry and reserved byte is zero. Decoders require the exact length.

Content identity is always:

```text
SHA256(exact_domain || exact_canonical_body)
```

The exact domains are:

```text
dragons-clutch/dealer-runtime/policy/v1\0
dragons-clutch/dealer-runtime/facility-genesis/v1\0
dragons-clutch/dealer-runtime/facility-position-binding/v1\0
dragons-clutch/dealer-runtime/facility-position/v1\0
dragons-clutch/dealer-runtime/root-tombstone/v1\0
dragons-clutch/dealer-runtime/liveness-schedule/v1\0
dragons-clutch/dealer-runtime/funded-dependencies/v1\0
dragons-clutch/dealer-runtime/runtime-liveness-binding/v1\0
dragons-clutch/dealer-runtime/action-liveness-receipt/v1\0
dragons-clutch/dealer-runtime/action-receipt-slot/v1\0
dragons-clutch/dealer-runtime/state/v1\0
dragons-clutch/dealer-runtime/lp-page/v1\0
dragons-clutch/dealer-runtime/lease/v1\0
dragons-clutch/dealer-runtime/settlement-pot/v1\0
dragons-clutch/dealer-runtime/fee-budget/v1\0
dragons-clutch/dealer-runtime/liveness-budget/v1\0
```

## Fixed body map

| Body | Magic | Bytes | Principal regions |
| --- | --- | ---: | --- |
| DealerPolicyV1 | `DCDPOLV1` | 1,148 | IDs, including the one neutral sink, `12..524`; width/padding `524..532`; scalar/array rules `532..1140`; page cap/reserved `1140..1148` |
| DealerFacilityGenesisV1 | `DCDFGNV1` | 116 | policy ID, sponsor, and refund recipient `12..108`; facility nonce `108..116` |
| FacilityPositionBindingV1 | `DCFPBND1` | 244 | facility/policy/Market/Position/account/Replay/State identities `12..236`; initial Position generation `236..244` |
| FacilityPositionBindingV2 | `DCFPBND2` | 220 | facility/policy/full Market/collateral-policy/collateral-release/State-controller identities `12..204`; fixed Dealer purpose byte plus zero padding `204..212`; founding PositionV3 generation `212..220`. Its purpose ID hashes exactly those non-cyclic facts and never either derived Position/Replay address; `DealerStateV2` solely persists both physical IDs. |
| DealerFacilityPositionV1 | `DCFPOSV1` | 388 | policy/facility/full Market/collateral/token/State authority/Replay identities `12..236`; phase/padding `236..244`; generation/cash `244..260`; exact Egg balances `260..388` |
| DealerRootTombstoneV1 | `DCRTMBV1` | 276 | policy/facility/Position binding/terminal State/root account/rent payer/sink identities `12..236`; terminal generation/child sequence and exact original rent split `236..276` |
| DealerLivenessScheduleV1 | `DCLSCHV1` | 372 | exact action mask/reserved `12..20`; maximum-call vector indexed by the frozen 22-action enum `20..196`; per-success lamport rewards `196..372` |
| DealerActionReceiptV1 | `DCACTRC1` | 532 | twelve exact Dealer/runtime/account identities `12..396`; action/compartment and runtime/Dealer generation, call ordinal, ceiling, payment, Replay ordinal `396..452`; deletable rent owner `452..532` |
| DealerFundedBudgetDependenciesV1 | `DCFDDEP1` | 412 | twelve exact policy/facility/schedule/runtime-policy/runtime-program/runtime-policy-account/fee/collateral/token/State/sink identities `12..396`; admission generation and exact six-compartment Dealer work principal `396..412` |
| DealerFundedDependenciesV2 | `DCFDDEP2` | 600 | nested immutable V1 dependency transcript `12..424`; non-cyclic PositionV3 purpose binding `424..456`; exact Initialize receipt account and semantic identities `456..520`; deletable rent owner `520..600` |
| DealerStateV1 | `DCDSTAT1` | 680 | IDs `12..364`; phase/disposition/width `364..372`; generation/child-sequence/share/sponsor facts `372..420`; signed `q[16]` `420..548`; eleven exhaustive counts `548..592`; root rent `592..680` |
| LpPageV1 | `DCLPPGV1` | 1,208 | policy/facility `12..76`; generation/chain/flags/revision `76..104`; 16 × 64-byte entries `104..1128`; child rent `1128..1208` |
| DealerLeaseV1 | `DCLSEV01` | 652 | 16 identities `12..524`; generation/deadlines `524..564`; width/row-count/padding `564..572`; child rent `572..652` |
| SettlementPotV1 | `DCPOTV01` | 1,084 | 12 identities `12..396`; phase/width `396..404`; generation `404..420`; row count/strict cursors `420..428`; `U_in/U_out/D_in/D_out` `428..460`; `F_buy/F_sell` `460..716`; liabilities `716..732`; collect/deliver totals `732..1004`; child rent `1004..1084` |
| FeeBudgetV1 | `DCFEEV01` | 348 | six identities `12..204`; generation and exact principal partition `204..260`; liability count/phase `260..268`; child rent `268..348` |
| LivenessBudgetV1 | `DCLIVV01` | 348 | same geometry as FeeBudget under a disjoint magic and content domain |

Every 80-byte deletable-child rent tail is
`payer[32] | neutral_sink[32] | refundable_principal u64 | donation_floor u64`.
The 88-byte root tail adds `permanent_tombstone_principal u64` between its
refundable principal and donation floor. The sink must equal the immutable
policy sink and must differ from the payer.

Pot phase bytes are only `0=Collecting`, `1=Delivering`, and `2=Finalizing`.
Finalize atomically sweeps the exact residue, applies the receipt, advances
generation, clears the root bindings/counts, and closes the Lease/Pot. A
post-sweep Pot is deliberately not serializable.

One LP entry is exactly:

```text
owner[32] | shares u64 | queued_shares u64 | terminal_claim u64 |
claimed u8 | reserved[7]
```

`terminal_allocated` is a page-level bit, so a legitimate zero-atom terminal
claim remains distinguishable from an allocation that has not happened.

## PDA seed preimages

These ordered seeds exclude the executing program identity and Solana bump.
All seed components are nonempty and at most 32 bytes.

| Family | Exact ordered seeds |
| --- | --- |
| Policy | `b"dc-dealer-policy-v1"`, `policy_id[32]` |
| Facility genesis | `b"dc-dealer-facility-v1"`, `facility_id[32]` |
| Facility Position binding | `b"dc-dealer-pos-bind-v1"`, `facility_id[32]` |
| Facility Position | `b"dc-dealer-position-v1"`, `facility_id[32]` |
| Facility Replay | `b"dc-dealer-replay-v1"`, `facility_id[32]` |
| Liveness schedule | `b"dc-dealer-live-sched-v1"`, `schedule_id[32]` |
| Funded dependencies | `b"dc-dealer-funded-v1"`, `facility_id[32]` |
| State | `b"dc-dealer-state-v1"`, `facility_id[32]` |
| LP page | `b"dc-dealer-lp-page-v1"`, `facility_id[32]`, `page_ordinal_le[4]` |
| Lease | `b"dc-dealer-lease-v1"`, `facility_id[32]`, `pre_generation_le[8]` |
| Settlement pot | `b"dc-dealer-pot-v1"`, `facility_id[32]`, `pre_generation_le[8]` |
| Fee budget | `b"dc-dealer-fee-v1"`, `facility_id[32]` |
| Liveness budget | `b"dc-dealer-live-v1"`, `facility_id[32]` |

The account address remains adapter-owned: it must be derived and
authenticated under the exact deployed program. No address in a semantic body
is accepted merely because these seed bytes can be reproduced.

## Sealed LP page-set fold

The activation fold consumes every adapter-authenticated page in exact ordinal
order. It refuses a gap, duplicate adjacent account, open page, terminal page,
cross-page owner reorder, broken tail, arithmetic overflow, or mismatch with
State's page/head/root/count/share/queue facts. The adapter must separately
derive each page key under the executing program; including the account key in
the transcript does not authenticate it.

The root is frozen as three SHA-256 stages:

```text
r0 = SHA256("dragons-clutch/dealer-runtime/lp-page-set/init/v1\0" ||
            policy_id || facility_id || counted_generation_le)
r[i+1] = SHA256("dragons-clutch/dealer-runtime/lp-page-set/step/v1\0" ||
                r[i] || page_account_id || page_ordinal_le || page_content_id)
root = SHA256("dragons-clutch/dealer-runtime/lp-page-set/final/v1\0" ||
              r[page_count] || page_count_le || live_entry_count_le ||
              total_shares_le || queued_shares_le)
```

`page_content_id` remains the exact mutable `LpPageV1` content identity, so a
queue, allocation, claim, revision, or rent-field mutation requires an atomic
new State root. The fold is an ephemeral checker result, never another
persisted truth owner.

## External runtime-liveness binding

`DealerRuntimeLivenessBindingV1` is an ephemeral streaming transcript, not a
persisted body or second budget. Its digest binds the funded-dependency body to
the separately authenticated seven-account runtime-liveness policy/bundle:
policy, Realm, facility lifecycle, sink, account/owner/payer/quote-schedule/
receipt-program IDs, funding classes, generations, exact work and rent
principals, maximum calls and per-call ceilings, admission before/after
balances, and four terminal-path call/work vectors. The external liveness
runtime remains sole owner of mutable calls, balances, receipts, refunds,
donations, and close transitions.

The persisted funded-dependency body separately pins the deployed liveness
program and physical immutable runtime-policy account. These are account
authority facts and therefore are not folded into the pure seven-body
projection; the live adapter must require that exact program to own the exact
policy account and all seven compartment accounts on every transition.

Each successful funded Dealer transition additionally authenticates one
`DealerActionLivenessAuthorizationV1`. Its semantic digest commits the action,
compartment, runtime account/owner/facility, quote schedule, receipt account and
program, runtime generation, Dealer economic generation, monotone call ordinal,
and exact action ceiling. The external runtime independently owns and checks
the ordinal, remaining work, keeper/refund transfers, and atomic account write.

The physical `DealerActionReceiptV1` is globally reserved at account
coordinate `0xa8/1` with an eight-byte global envelope followed by the exact
532-byte body. Its PDA is
`[b"dc-dealer-action-receipt-v1", receipt_slot_id]`, where `receipt_slot_id`
commits the facility, runtime account, Replay account, action/compartment,
both generations, runtime call ordinal, and Replay ordinal. The postimage also
owns the actual keeper payment and an explicit deletable-rent split; neither is
work-capital custody. Only an adapter-authenticated exact postimage may project
the generic runtime intent and receipt observation.
The counted `DealerFundedDependenciesV2` child co-owns the Initialize receipt's
exact account and semantic identities. Its terminal close authenticates and
deletes both rent-owned accounts atomically after the Position, Replay, Epoch,
LP, lease, pot, allocation, and claim families are exhausted; Initialize
therefore cannot leave an uncounted receipt or refundable principal behind.

The Dealer schedule is finer grained. Actions map as follows:

| External compartment | Dealer actions |
| --- | --- |
| Source | none; separately funded external schedule is mandatory |
| Candidate | BindEpoch, LapseEpoch, SelectLeaseAndBegin |
| Clearing | Initialize, CreateLpPage, Activate |
| Settlement | Collect, Deliver, FinalizeSettlement, Claim |
| Resolution | Resolve |
| Retirement | EnterUnwind, TimedClose, Retire; QueueExit only if explicitly promised |
| Recovery | CancelFunding, RefundCancelledSponsor, AbortBeforeCollection |

CreatePolicy is outside the facility lifecycle. Contribute, WithdrawFunding,
and SponsorHalt are signer/caller-funded and cannot appear in the prepaid
Dealer work dot product. Rent is separately owned by the external accounts and
is never included in that dot product.

## Frozen vector identities

The hostile vector corpus fixes the following sample digests:

| Body | SHA-256 content ID |
| --- | --- |
| Policy | `7db0f47420b59c7b720bccd1e54fd6d493d6540da1600b9e8e0f8cbc43dc1231` |
| Funding State | `7b13c2414fa15f7847390c2801a7833f218f73efd37fc3b0c7441542b9d0bdd9` |
| LP page | `11ecfc8a9be38f83fe683511563a64d452eba56684caeef1832af9bfc1d0baa6` |
| Lease | `fb1dfa7996c21a90b2ad5c98dac6b21d023616ff9b94568254187c0a8fb508f4` |
| Finalizing pot | `f92188d05c37437a3e2297d40061ab2ead7363761504802eac7103d876fd8e1c` |
| Fee budget | `85aaf8a3941438b1c06232316fe0842abc40b57b629c85b718def069f9e149ff` |
| Liveness budget | `da8be0b9ba81cb3b67e361b95bd270c364a42837d2819c13adae11e4928b1ed8` |

Any field order, width, endianness, magic, version, padding, or content-domain
change must use a successor version and new frozen vectors.
