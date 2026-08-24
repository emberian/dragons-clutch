# Dealer catalog and facility-foundation SBF vertical slice

Status: **EXPLICITLY NON-PRODUCTION / SELF-HOSTED LIVENESS, BOUNDED LP FUNDING,
ACTIVATION/CANCELLED REFUND, AND BOUNDED EPOCH BIND/LAPSE / NO ORDER OR
SETTLEMENT CAPABILITY**.

The signed resumable catalog persists exactly one typed `DealerPolicyV1`,
`DealerLivenessScheduleV1`, or generic `RuntimeLivenessPolicyV1` body. The
separate facility adapter owns exact Initialize, bounded LP-page creation,
contribution, pre-activation withdrawal, activation, stale-funding cancellation,
exhaustive cancelled-sponsor refund, and BindEpoch transitions. It does not
enable order admission, selection, settlement, claims, or ordinary retirement.
An unused bound epoch may be lapsed after its frozen deadline with exhaustive
zero Lease/Pot counts.

The separate profile identity is:

```text
dragons-clutch/capability-profile/non-production-dealer-self-hosted-liquidity-refund-bind-lapse-lab/v8
b367e8434138e6073b46f539664d31a7296e88d65b027572004e106fcd54fd8b
```

Every production profile rejects these Dealer coordinates before account
inspection. The laboratory profile rejects every legacy intent and enables
only Dealer family 76, version 1, local actions `1..=13`.

## Wire and account contract

| action | payload | replay owner |
| --- | ---: | --- |
| `BeginPolicy` (1) | kind 1 + zero pad 7 + ID 32 + neutral sink 32 + expiry slot 8 | outer sequence exactly zero and absent stage PDA |
| `WritePolicy` (2) | kind/pad 8 + ID 32 + cursor 2 + active length 2 + padded chunk 192 | outer sequence, payload cursor, and stored cursor equal |
| `SealPolicy` (3) | kind/pad 8 + ID 32 | sequence and stored cursor equal the selected body's exact length |
| `AbortPolicy` (4) | kind/pad 8 + ID 32 | sequence equals stored cursor |

The stage is `0x7d/1`, exactly `140 + 1,148 = 1,288` bytes, at:

```text
[b"dc-dealer-policy-stage-v1", artifact_kind_u8, funder, artifact_id]
```

Its header owns the artifact kind and full ID, funder, neutral sink, stored
bump, strict cursor, selected exact length, creation/expiry slots, full refundable rent principal,
and hostile creation prefund. The body is zero-initialized and only the next
strict 192-byte chunk (or final 188 bytes) may be written. Inactive chunk bytes
must be zero.

Policy seals to immutable `0x7e/1`, exactly `56 + 1,148 = 1,204` bytes.
Schedule seals to immutable `0x93/1`, exactly `8 + 372 = 380` bytes. The
generic runtime policy remains its exact raw 1,132-byte codec so the canonical
liveness adapter can decode it without a parallel DTO. Their PDA recipes are:

```text
[b"dc-dealer-policy-v1", policy_id]
[b"dc-dealer-live-sched-v1", schedule_id]
[b"dc-dealer-runtime-liveness-policy-v1", runtime_policy_id]
```

The policy adapter header owns the stored bump, catalog funder, permanently
locked rent principal, and creation-time final-PDA donation. Every body remains
its one semantic truth. Seal hostile-decodes the selected kind and recomputes
its frozen identity. Policy uses
`SHA256("dragons-clutch/dealer-runtime/policy/v1\0" || body)`, and requires
the result to equal the request, stage, and PDA identity.

## Ordered account metas

- Begin: funder signer/writable; stage writable; System executable/read-only;
  Rent read-only; Clock read-only.
- Write: funder signer/read-only; stage writable; Clock read-only.
- Seal: funder signer/writable; stage writable; final writable; neutral sink
  writable; System executable/read-only; Rent read-only; Clock read-only.
- Abort: caller signer/read-only; stage writable; stored funder writable;
  stored neutral sink writable; Clock read-only. Before expiry caller must be
  the funder; after expiry any non-conflicting signer may reap.

Payer, stage, final, and sink identities are authenticated and forbidden alias
classes are checked explicitly. Program ownership, executable bits,
writability, exact lengths, tags, versions, padding, stored bump, PDA, and
Clock/Rent/System identities all fail closed.

## Rent and hostile prefunds

Stage and final creation use full-principal funding. A one-lamport predictable
PDA squat never discounts the funder's principal.

- Stage balance after Begin is `hostile_prefund + exact_stage_rent`. At Seal or
  Abort the funder receives exactly `exact_stage_rent`; the creation prefund
  and every later surplus go to the stored policy neutral sink.
- A prefunded final PDA is PDA-signed back to zero and credited to that sink
  before allocation. The funder then supplies the full final rent minimum.
  The immutable catalog retains exactly that principal.

Hoard principal, collateral, LP assets, fees, future fee revenue, and any
economic Dealer budget are never rent sources.

## Evidence and remaining blockers

The real-ELF SVM campaign covers successful Begin/Write/Seal/Abort, strict
replay, full account-image invariance on refusal, transaction-wide rollback of
a prior System transfer, wrong-Rent and owner/sink identity substitutions,
one-lamport stage and final squats, exact owner/PDA/body/creation rent, sink
routing, and duplicate Seal refusal. It also submits a valid legacy Split to
prove the laboratory profile refuses it before accounts. The production ELF
separately proves pre-account rejection of the same allocated Dealer request.
No mock-source account, feature, parser, fixture, or dependency participates
in this route.

The laboratory now has exact Initialize, `CreateLpPage`, `Contribute`,
`WithdrawFunding`, `Activate`, `CancelFunding`, `RefundCancelledSponsor`,
`BindEpoch`, and `LapseEpoch` handlers over canonical PositionV3, ReplayV3,
funded-dependency, action-receipt, LP-page, General Epoch, and runtime-liveness
owners. The immutable schedule and generic runtime policy can be published
through this same catalog rather than injected as fixture DTOs.
Initialize atomically creates all seven canonical runtime compartment PDAs from
exact present native-lamport work and rent debits. Hostile prefunds remain
neutral-sink donations and never discount the liveness payer.

`CreateLpPage` consumes the Clearing compartment and one immutable typed action
receipt. The first-page route uses 20 ordered accounts; successor creation uses
21 and additionally authenticates and seals the current full tail. Page PDAs are
`[b"dc-dealer-lp-page-v2", facility_id, page_ordinal_le]`. State owns the page
count and current page-set root; the mutable tail owns its bounded entries and
the predecessor owns the sealed successor link. Receipt and page rent are paid
from the current actor, while Clearing keeper/refund lamports retain the
immutable compartment payer. Any balance surplus received since the preceding
call is first projected through the canonical donation-observation transition,
so it cannot stall a funded action or become work principal.

Contribution and withdrawal are caller-funded Replay transitions over seven
ordered accounts. The adapter authenticates the actor as the controller of one
ordinary PositionV3 and records that Position's owner—not the signer address—as
the LP share owner. Cash and native-Egg deltas are derived only from the
immutable capital unit times the exact share delta. They move internally between
the LP Position and facility Position; no token CPI, Hoard mutation, liveness
debit, fee source, or caller-shaped asset vector participates. The mutable tail
page owns sorted LP entries while State owns aggregate shares, live-owner count,
current facility Position semantic ID, and page-set root. Both Position bodies,
the page, State, and Replay advance atomically.

Activation consumes a typed Clearing receipt only while Clock is inside the
immutable trading window. It authenticates the last mutable tail and requires
the exact facility Position cash/Egg vector to equal sponsor capital plus every
admitted capital unit. It then seals the tail, moves State from Funding to
Trading, makes sponsor capital non-refundable under the selected policy, and
advances Replay atomically with the donation-safe Clearing spend. A partial
page set, insufficient shares, reserved cash, excess/short collateral, a stale
Clock, or a substituted liveness payer cannot activate the facility.

Stale-funding cancellation consumes the Recovery compartment and moves only a
still-Funding State into Cancelled. Clock must prove either the trading window
has ended or the opening slot arrived without the immutable minimum LP shares.
It does not move sponsor/LP collateral, close a page, or treat the Recovery
payment as a refund: LP owners first withdraw their exact capital units through
the ordinary caller-funded route, and sponsor-principal return remains a
separate typed transition after exhaustive LP/page cleanup.

Cancelled sponsor refund is permissionless keeper work but its destination is
not caller-selected: the ordinary PositionV3 owner must equal State's immutable
sponsor-refund recipient. Only the exact recorded sponsor principal moves from
the facility Position, and the pure transition additionally requires zero LP
shares, zero live LP owners/pages, zero Eggs/reserved cash, and a zero facility
Position post-balance before State can become Retiring. Recovery keeper payment,
receipt/page rent, hostile donations, Hoard principal, and fees are disjoint
from this collateral-atom transfer.

Epoch lapse is a separately funded Candidate action and is permissionless only
after the retained binding's exact lapse slot. The adapter authenticates the
full seven-compartment runtime bundle, the original Bind receipt, the counted
Epoch PDA, State, PositionV3, and ReplayV3. The pure owner requires the Epoch to
remain Bound with zero Lease/Pot children. Lapse advances the Position and
State generation without changing any collateral or reservation field, clears
the sole active Epoch child, and advances Replay atomically. The Epoch's and
Bind receipt's recorded principals return only to their recorded payers; their
creation prefunds and every later donation go only to the immutable neutral
sink. The fresh lapse receipt remains as replay-referenced evidence.

Every other Dealer facility action remains capability-disabled, including
selection, collection, delivery, resolution, claims, and retirement.

Run the real-bank laboratory with:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  --profile-non-production-dealer-policy-catalog-lab \
  real_sbf_catalog_is_resumable_rent_exact_and_replay_safe
```

The same checkout's production refusal is:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  production_profile_refuses_allocated_dealer_action_before_accounts
```
