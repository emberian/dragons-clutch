# Structured-claim SBF successor adapter

Status: **complete disabled runtime seam; not routed, built, measured, deployed,
or validated** (2026-08-23).

This crate consumes `clutch-structured-claim-runtime-contract` as the only
owner of structured-claim descriptor bytes, family-local payload codecs, and
economic transitions. The former adapter-local descriptor (`0xd1`), request,
wrapper-replay, route planner, backing reconstruction, and post-state DTOs were
deleted. There is no compatibility decoder: a historical parallel-adapter
image cannot be mistaken for the canonical descriptor.

The canonical persisted descriptor is exactly 384 bytes at account coordinate
`0x88/1`. It contains deployment identity, Market/Terms identity, primitive
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

- central registry allocation of the eight family-local actions and descriptor
  account `0x88/1`;
- an exact capability-profile tuple and new profile/release identity;
- main-dispatcher routing to this crate;
- concrete base CPI instructions named below;
- a pinned Token-2022 byte parser and CPI encoder;
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

- the central registry currently reserves the structured family but must still
  allocate its eight local actions and descriptor `0x88/1`;
- the base program does not yet expose the authenticated empty-vault creation,
  atomic Position asset transfer, atomic full-vector wrap/unwind,
  beneficiary-free compaction, exact terminal redemption, and close receipt
  interfaces staged here;
- the main SBF program's pinned Token-2022 parser/CPI helpers must be extracted
  or exposed without creating a second Token-2022 truth;
- the main dispatcher has no structured-claim account loader or route arm; and
- no successor build, measurement, bank, SVM, local-validator, or rollback
  campaign has run. `SBF_EVIDENCE.md` records that explicit evidence state.

These are activation dependencies, not reasons to retain the deleted duplicate
planner or to describe this family as live.
