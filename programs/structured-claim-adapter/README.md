# Structured-claim SBF successor adapter

Status: **implemented disabled source seam; not routed, built, measured,
deployed, or validated** (2026-08-23).

This crate consumes `clutch-structured-claim-runtime-contract` as the only
owner of structured-claim descriptor bytes, family-local payload codecs, and
economic transitions. The former adapter-local descriptor (`0xd1`), request,
wrapper-replay, route planner, backing reconstruction, and post-state DTOs were
deleted. There is no compatibility decoder: a historical parallel-adapter
image cannot be mistaken for the canonical descriptor.

The former legacy `solana-layout::PositionAccount` plus
`solana-reference::ReplayAccount` authentication path is also gone. Mutation
inputs now authenticate the canonical 480-byte Position V3, the shared
purpose-owned Replay V3 envelope, and the exact `GEN1` or `SCV1` extension
before projecting the runtime contract's small economic view.

The canonical persisted descriptor is exactly 384 bytes at account coordinate
`0x88/1`. It contains deployment identity, MarketInstanceV2/NativeClaimBasis
identity, primitive
native-Egg coefficients, lifecycle, and PDA bumps. Actual wrapper supply is
always the extension-free Token-2022 mint. Backing is always the dedicated
base Position. Hoard, native supply, Market phase, and payouts remain base
program facts.

## Runtime activation is empty

The structured family is `75/v1`, with eight runtime-contract actions:

1. create descriptor;
2. canonical wrap;
3. full-vector wrap;
4. canonical unwind;
5. full-vector unwind;
6. beneficiary-free donation compaction;
7. exact terminal redemption; and
8. permanent descriptor retirement.

`ENABLED_STRUCTURED_CLAIM_ACTION_MASK` is zero. Runtime admission reads only
the three-byte family/version/action header and refuses every allocated action
before payload or account data is read. The pure planners are implementation
contracts, not an execution capability.

Activation must be atomic with all of the following:

- central capability activation for the already allocated eight family-local
  actions and descriptor account `0x88/1`;
- an exact capability-profile tuple and new profile/release identity;
- main-dispatcher routing to this crate;
- a live base handler for the concrete Position V3 action-35 CPI below;
- the supplied parser plans wired into the pinned Token-2022 byte parser;
- linked ELF, stack, heap, compute, CPI-depth, account-count, rollback, rent,
  SVM, and local-validator evidence; and
- a checked release manifest.

## Trust-boundary responsibilities

The adapter owns only facts that cannot live in the pure runtime contract:

- exact wrapper/base/Token-2022 Program and ProgramData ownership, linkage,
  executable bits, and deployment slots;
- SHA-256 of runtime-owned native-claim and wrapper-product preimages;
- descriptor, mint, mint-authority, and vault-owner PDA authentication;
- exact account-role ordering, access, program ownership, and pairwise
  nonaliasing;
- hostile decoding of canonical base Market, Terms, Hoard, SupplyLedger,
  Position, and current-generation Replay accounts;
- projection through a named base-PDA verifier;
- projection through a named pinned Token-2022 parser; and
- exact ordered outer CPI/write plans plus receipt reconciliation.

`Token2022DecoderV1` is deliberately a trust boundary, not a convenient mock
codec. Its implementation must use the pinned Token-2022 layout and reject
every mint extension, nonzero decimals, freeze authority, wrong mint authority,
or uninitialized mint. Holder accounts must reject frozen, native, delegated,
close-authority, wrong-mint state, and every extension except
`ImmutableOwner`. Runtime-contract projections are accepted only after that
parser returns them, and the runtime rechecks the fields it owns.

`wrapper_mint_parser_plan_v1` and `wrapper_token_parser_plan_v1` make those
requirements exact. `plan_token_2022_cpi_v1` emits the real Token-2022 byte
layouts and metas for `InitializeMint2`, `MintToChecked`, `BurnChecked`, and
`SetAuthority(MintTokens, None)`; it is not a mock execution result.

## Ephemeral structured custody

Canonical wrap/unwind uses General V2 family `74/v1`, local action 35, with
the canonical 298-byte payload owned by
`clutch-structured-claim-runtime-contract`. `authority_id` is a SHA-256 digest
over domain
`dragons-clutch/authenticated-structured-custody-call/v1\0` and the exact
1,352-byte authority-neutral projection. It binds action 35, the exact local
wrapper action, descriptor/product/deployments, MarketBinding and Product
artifacts, the complete canonical base Market prestate and current lifecycle,
Realm collateral-policy/release identities, user actor, vault PDA, both
complete Position V3 semantic prestates, both complete Replay prestates, and
the exact transfer delta.

There is no generic capability account and no wildcard signer. The wrapper
prepares the call and the base reconstructs it through the same function. The
frozen action-35 CPI metas are:

0. vault-owner PDA, signer, read-only;
1. immutable General V2 MarketBinding, read-only;
2. source Position V3, writable;
3. source purpose-owned Replay V3, writable;
4. destination Position V3, writable;
5. destination purpose-owned Replay V3, writable;
6. user Position controller, signer, read-only;
7. immutable `0x88/1` descriptor, read-only;
8. pinned wrapper executable, read-only;
9. pinned wrapper ProgramData, read-only;
10. pinned base executable, read-only;
11. pinned base ProgramData, read-only;
12. pinned Token-2022 executable for the wrapper plane, read-only;
13. pinned Token-2022 ProgramData, read-only;
14. exact NativeClaimBasisV1 Product artifact, read-only; and
15. exact MarketInstanceV2 preimage account, read-only; and
16. canonical base Market carrying current lifecycle, read-only.

The vault signer proves that only the pinned wrapper program could have
produced `invoke_signed`; the digest prevents that signer from becoming
authority for any other action, deployment, product, account pair, generation,
sequence, or delta. The Realm-selected collateral adapter is consumed as a
private-field `BoundCollateralProfileV2`; this module neither reparses
collateral tokens nor invents a second collateral release truth.

Both Position mutations stage exact 480-byte Position V3 successors and must
atomically advance their canonical purpose-owned Replay V3 envelopes. The
General endpoint uses the single General `GEN1` extension shared by settlement
and custody actions. The vault endpoint uses StructuredClaim schema `SCV1`,
whose immutable descriptor/product/vault join and current Position semantic ID
cannot be detached from its last transition and delta digests. The structured
delta commits both consumed ordinals, both Position accounts, and both exact
pre/post Position semantic IDs; the transition digest already commits the full
action-35 payload and complete authenticated account projection.
`GEN1.transition_id` is that authenticated custody digest and its
`transition_evidence_id` is the rederived `SCV1` structured delta. The General
owner then derives its own endpoint delta over that evidence, its exact Position
pre/post IDs and generation, the consumed ordinal, and the exhaustive
StructuredGeneral/action-35 tuple.

`StructuredCustodyScratchV1` owns the 2,352-byte NativeClaimBasis decode target
and 1,352-byte authority transcript. A live SBF entrypoint must place this
scratch on requestable heap storage and pass it to preparation/reconstruction;
the bridge does not return those large values through the 4-KiB stack. The
adapter hashes the exact canonical Product and MarketBinding account bytes after
their hostile decoders accept them, avoiding redundant large re-encoding.

## Exact route staging

Every route calls the canonical runtime function first, on copies, before
returning an execution plan. The base CPI variants carry the returned runtime
plan itself rather than an adapter copy of its post-state.

| action | ordered outer operations |
| --- | --- |
| create | descriptor System allocation; mint System allocation; InitializeMint; base empty-vault creation; descriptor write |
| canonical wrap | base atomic Position transfer; MintToChecked |
| full wrap | base atomic full-vector custody + complete-set compression; MintToChecked |
| canonical unwind | BurnChecked; base atomic Position return |
| full unwind | BurnChecked; base atomic complete-set expansion + full-vector return |
| compact donation | base atomic beneficiary-free cash/native-Egg donation |
| terminal redemption | BurnChecked; base exact terminal aggregate redemption |
| retirement | authenticated base vault close; Token-2022 mint-authority revocation; descriptor tombstone write |

The full-vector, compaction, and terminal routes require one atomic base
instruction each. Splitting them into independent transfer/merge/donation calls
would consume inconsistent Replay sequences and expose partial semantic
authority. Solana transaction rollback remains necessary but is not accepted
as a substitute for checking the exact final Market, Hoard, supply, Position,
Replay, mint, token, descriptor, and tombstone fields in the dispatcher.

## Rent and prefunding

Descriptor and mint construction uses the runtime contract's exact current-bank
rent shortfall plan. Existing lamports stay locked in the permanent descriptor
and mint identities and never become a refund, fee, bounty, reserve, treasury,
or caller claim.

The closable base Position/Replay pair is separate. A base-owned creation
capability must bind creator-funded shortfalls, hostile/benevolent prefunds, a
beneficiary-free neutral sink, and one `rent_transition_id`. Its later close
capability must return only the creator-funded component to that creator and
send all prefunding to the neutral sink. The wrapper cannot mint either
capability from caller-authored fields.

## Remaining external dependencies

The adapter implementation is intentionally honest about work owned elsewhere:

- the base program does not yet route the authenticated Position V3 action-35
  handler, empty-vault creation, atomic full-vector wrap/unwind,
  beneficiary-free compaction, exact terminal redemption, and close receipt
  interfaces staged here;
- the main SBF program must consume the exact Token-2022 parser and CPI plans
  without creating a second Token-2022 truth;
- the main dispatcher has no structured-claim account loader or route arm; and
- no successor build, measurement, bank, SVM, local-validator, or rollback
  campaign has run. `SBF_EVIDENCE.md` records that explicit evidence state.

These are activation dependencies, not reasons to retain the deleted duplicate
planner or to describe this family as live.
