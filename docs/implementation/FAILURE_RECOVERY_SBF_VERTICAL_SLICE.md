# Disabled Failure/Recovery SBF vertical slice

Status: ABI and account contract frozen; execution capability disabled.

This document fixes the main-program boundary for the single-custody failure
runtime. It does not promote a deployable route. The central capability table
contains no Recovery tuple, so family `78`, version `1`, actions `1..=9` refuse
before any account is read.

## Semantic ownership

The boundary has four persisted families and no parallel economic ledger:

| tag/version | account | semantic owner | exact bytes |
|---:|---|---|---:|
| `0xa0/1` | external failure root | `clutch-failure-policy-runtime` through `clutch-failure-policy-adapter` | 2,156 |
| `0xa1/1` | immutable liveness policy | `clutch-liveness` | 1,136 |
| `0xa2/1` | external Recovery compartment | `clutch-liveness` | 468 |
| `0xa3/1` | permanent replay tombstone | terminal/replay adapter | 256 |

The first three accounts have a four-byte main-program frame `(tag, version,
bump, zero flags)` followed by the complete semantic-owner body. The adapter
passes the body slice to its owner; it does not decode or copy selected inner
fields. The tombstone has its complete hostile codec in
`clutch_solana_layout::failure_recovery`.

The root owns failure identity, immutable trigger policy, exact finite repair
schedule, transition nonce, generation, accepted evidence identities, and
terminal classification. Its lamports contain only its separately tracked
account-rent principal plus unsolicited surplus.

The `0xa2` Recovery account is the sole work/rent custody. Accepted work debits
its scheduled ceiling, pays the exact runtime-selected keeper reward, and
refunds `ceiling - reward` to its immutable payer. The root's lamport balance is
unchanged. A transaction which writes only one of these two poststates is not a
valid accepted-work transaction.

No Hoard principal, collateral principal, future fee, market fee carry, Series
allocation, or root rent is recovery work capital. The `0xa1` policy and `0xa2`
compartment must prove present funding under their own exact codecs.

## Address allocation

The PDA domains are:

- root: `("dc:failure-root:v2", MarketInstanceV2Id, generation_le)`;
- liveness policy: `("dc:failure-live-policy:v1", policy_id)`;
- Recovery compartment: `("dc:failure-recovery:v1", lifecycle_id,
  generation_le)`;
- tombstone: `("dc:failure-replay:v1", MarketInstanceV2Id, generation_le)`.

The adapter checks both the derived address and the stored frame bump. It also
checks the root body's semantic state ID and the liveness body's account ID
against the physical addresses. Root, policy, Recovery, payer, keeper, neutral
sink, and terminal roles may not alias where their semantics differ.

Tags `0x93..=0x9e` remain outside this wave for the coordinated Dealer design.
Tag `0x9f` is intentionally unallocated. Recovery begins at `0xa0` and ends at
`0xa3`.

## Strict action payloads

Every payload begins with:

| offset | bytes | field |
|---:|---:|---|
| 0 | 32 | nonzero `FailurePolicyBindingId` |
| 32 | 32 | nonzero full-width `MarketInstanceV2Id` |
| 64 | 8 | nonzero generation, little-endian |
| 72 | 8 | exact expected root transition nonce, little-endian |

Action-specific tails and exact total lengths are:

| local action | total | tail |
|---:|---:|---|
| 1 InitializeFailureRoot | 160 | SeriesPlanV5 ID; ordinal; 4 zero bytes; SeriesFundingQuote ID; root-rent principal |
| 2 TriggerSourceFailure | 112 | Source failure-handoff ID |
| 3 TriggerRelationRefusal | 152 | Source success-handoff ID; relation-record ID; refusal `u32`; 4 zero bytes |
| 4 AdvanceRecoverySchedule | 88 | expected attempt `u8`; 7 zero bytes |
| 5 AcceptRecoveryWork | 152 | Source success-handoff ID; reward recipient; scheduled ceiling |
| 6 ResolveCallerFunded | 144 | Source success-handoff ID; accepted relation-record ID |
| 7 ResolvePaidRecovery | 184 | Source success-handoff ID; accepted relation-record ID; reward recipient; scheduled ceiling |
| 8 CloseRecoveryFunding | 112 | Recovery terminal-receipt ID |
| 9 CloseFailureRoot | 208 | full failure-terminal join; retirement root; replay tombstone; final Source release receipt |

All IDs are nonzero. Generation, root-rent principal, and scheduled ceilings are
nonzero. Relation refusal codes are the closed range `1..=5`. Decoders reject
short, trailing, noncanonical padding, zero identity, zero required amount, and
wrong-enum inputs. Payload fields are expected values, never authority by
themselves.

## Account contracts

The exact ordered role arrays live beside the payload codec. Important joins
are summarized here.

Initialization consumes the root-rent payer and fresh root, the immutable
liveness policy, already funded read-only Recovery compartment, immutable
neutral sink, authenticated
Series registry/funding state, central registry release/profile, all nine
immutable Product/Series artifacts, complete Source release/deployment/config
chain, the exact occurrence/window/statistic identities, the Clock and Rent
sysvars, and the System program. It recomputes `CompiledOrdinalV2` and invokes
`FailureRuntimeExternalV2::admit_successor`; it does not accept a caller-built
admission DTO. The root is created only after the liveness compartment's actual
lamports equal its decoded accounted balance and the failure admission receipt
names the same Series, ordinal, market, quote, root, policy, lifecycle,
Recovery account, payer, sink, and generation.

Source-trigger, relation-trigger, work, and resolution instructions consume an
authenticated Source release, occurrence, result/absence, and Source work
receipt. The Source owner reconstructs `FailurePolicySourceHandoffV1` or
`SuccessfulEvaluationHandoffV1` from an
`AuthenticatedStatisticResultAccountV1` or exact authenticated absence. The
failure adapter consumes those private-field facts and an
`AuthenticatedSourceWorkReceiptV1`, obtains the physical result and receipt
accounts only from their typed getters, and requires the persisted work
receipt's semantic receipt to be the exact handoff ID named by the payload. A
relation result is separately authenticated under the immutable relation policy
committed by the root. It may classify only accepted versus one of five
refusals and never chooses a payout.

Every time-sensitive action reads the canonical Clock sysvar directly. The
adapter checks its exact address, sysvar owner, read-only/non-signer/non-
executable privileges, exact 40-byte body, nonnegative Unix timestamp, and
derives the bucket from the exact ClockPolicy embedded in the authenticated
Source release. Caller-supplied timestamps, buckets, and policy-ID shadows are
absent from the ABI.

## Atomic work and close handlers

For accepted work or paid resolution, the handler performs all checks before
the first mutation:

1. authenticate root owner/frame/PDA/body/digest/rent coverage and exact common
   binding, market, generation, and nonce;
2. authenticate Source success and, when required, relation acceptance;
3. authenticate liveness policy and mutable Recovery body/frame/PDA/balance;
4. ask failure semantics for the transition and typed work receipt;
5. project that receipt into liveness and require equality of recipient,
   exact reward, scheduled ceiling, quote schedule, owner, lifecycle,
   generation, call ordinal, and receipt account/program;
6. require `exact_reward <= scheduled_ceiling`, Recovery balance decrease by
   exactly the ceiling, keeper increase by exactly the reward, and immutable
   payer increase by exactly the unused headroom;
7. write the root and Recovery poststates and apply exactly those two liveness
   movements in the same outer Solana instruction.

There is no root debit and no second keeper movement.

`CloseRecoveryFunding` authenticates the current failure Recovery terminal
receipt. Resolved maps to liveness `TerminalSuccess`; dormant maps to liveness
`TerminalFailure`. Both close only `0xa2` and dispose its remaining work, rent,
and donations under liveness policy. Dormancy is not Retirement success and
does not authorize root closure.

`CloseFailureRoot` is separately gated by resolved failure state, a closed
Recovery PDA observation, authenticated retirement root, permanent replay
tombstone, and final Source release/lineage receipt for the same binding,
MarketInstanceV2, and generation. It refunds only the root's recorded rent
principal to its immutable payer and sends root-only surplus to its immutable
neutral sink. It never moves liveness or collateral funds.

The Recovery-terminal receipt commits a domain-separated hash of all 2,032
canonical failure-runtime bytes, not merely the stable state-account identity
and nonce. The full terminal join then commits that exact resolved receipt and
the current root transition nonce. The pure close projector and SBF handler
independently recompute both from the authenticated root, so an older or sibling
terminal join cannot authorize a different resolved poststate in the same
binding and generation.

The replay tombstone stores nonzero present permanent rent, the exact funder and
typed funding-admission receipt, any pre-existing lamports as a distinct locked
donation, binding, market, generation, full terminal join, retirement root, and
final Source release receipt. Its semantic owner funds and persists it before
root closure. The admitted debit must equal the recorded rent principal, and
the observed post-balance must equal prior donation plus that debit. The typed
funding owner must exclude Recovery work principal, Hoard principal, collateral,
and future fees.

## Capability state

The codecs, PDAs, account tags, and handler plans are not an activation signal.
`ENABLED_EXTENSION_ACTIONS` contains no `(78, 1, action)` tuple. Dispatch's
canonical disabled-coordinate guard returns `UnsupportedInstruction` before
account inspection for all nine actions. Promotion requires one explicit
release change which admits the complete family and its cross-owner adapters;
individual partial action promotion is not permitted.
