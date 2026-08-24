# Structured-claim SBF successor adapter

Status: **current action owners are staged behind exact zero capability masks;
neither the wrapper nor the base program admits a Structured action, and no
artifact has been built, measured, deployed, or validated** (2026-08-24).

This crate consumes `clutch-structured-claim-runtime-contract` as the owner of
structured-claim descriptor bytes, family-local payload codecs, roots, replay
extensions, and terminal plans. The adapter's current lifecycle composes only
hostile-decoded HoardV2, ClaimLedgerV3, PositionV3, ReplayV3, ResolutionV5,
Product, and collateral authorities; the former parallel `MarketLedger` and
model route planner have been physically deleted. The former adapter-local descriptor (`0xd1`), request,
wrapper-replay, route planner, backing reconstruction, and post-state DTOs were
deleted. There is no compatibility decoder: a historical parallel-adapter
image cannot be mistaken for the canonical descriptor.

The former legacy `solana-layout::PositionAccount` plus
`solana-reference::ReplayAccount` authentication path is also gone. Mutation
inputs now authenticate the canonical 480-byte Position V3, the shared
purpose-owned Replay V3 envelope, and the exact `GEN1` or `SCV1` extension
before projecting the runtime contract's small economic view.

Historical descriptor v1 is exactly 384 bytes at `0x88/1` and remains
decode-only. The sole future descriptor v2 is exactly 449 bytes at `0x88/2`.
It additionally binds the exact Series-scoped Structured root and authenticated
wrapper recipe, while retaining distinct descriptor/mint/mint-authority/vault
bumps. The mutable `0xb7/1` root owns Product lineage, descriptor counts,
ordered admission/terminal transcripts, refundable rent principal, and
donation residue. Actual wrapper supply is always the extension-free
Token-2022 mint; backing is always the dedicated base Position. Hoard, native
supply, Market phase, and payouts remain base-program facts.

## Runtime activation

The structured family is `75/v1`, with eight runtime-contract actions:

1. create descriptor;
2. canonical wrap;
3. full-vector wrap;
4. canonical unwind;
5. full-vector unwind;
6. beneficiary-free donation compaction;
7. exact terminal redemption; and
8. permanent descriptor retirement.

The adapter default remains fail-closed. The historical name
`live-current-wrapper` stages current source but its release-admitted action
mask is exactly zero, as is the base program's Structured mask. All eight
actions therefore refuse at capability admission before account loading.

Actions 1, 3, 5, 6, 7, and 8 have one shared source/account contract used by
both wrapper and base: founding uses 34 accounts, full-vector wrap/unwind use
32, compaction uses 32, terminal redemption uses 33, and retirement uses 31.
Their exact token effects are permanent-mint initialization, mint, burn,
optional Hoard-surplus disposition, burn, and mint-authority revocation. The
withdrawn canonical actions 2 and 4 have no current execution contract. All
eight actions remain ReservedDisabled because the distinct release-admitted
mask is zero. The exact table is content-addressed by
`STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1`; current wrapper/base/Token-2022
release authentication consumes the single aggregate release contract.

Activation must be atomic with all of the following:

- central capability activation for only the implemented family-local actions,
  descriptor account `0x88/2`, and root account `0xb7/1`;
- an exact capability-profile tuple and new profile/release identity;
- main-dispatcher routing for only the admitted current actions;
- linked ELF, stack, heap, compute, CPI-depth, account-count, rollback, rent,
  SVM, and local-validator evidence; and
- a checked release manifest.

## Trust-boundary responsibilities

This adapter stays separate from `programs/clutch-sbf` deliberately. The
descriptor, wrapper mint, mint-authority PDA, wrapper executable, and
Token-2022 post-CPI observations belong to the wrapper deployment, while the
base program alone owns Position V3 and Replay writes. Linking the adapter as
a shared `no_std` library gives both entrypoints one exact account-count,
release-manifest, Token-2022, and successor-projection contract without moving
wrapper authority into the base ELF or inventing a second persisted truth. It
does **not** authorize either side to trust the other's projection: each side
hostile-authenticates the accounts it owns and reconciles the exact joined
postimages before success.

The adapter owns only facts that cannot live in the pure runtime contract:

- exact wrapper/base/Token-2022 Program and ProgramData ownership, linkage,
  executable bits, and deployment slots;
- SHA-256 of runtime-owned native-claim and wrapper-product preimages;
- descriptor, mint, mint-authority, and vault-owner PDA authentication;
- the shared current action/account-count contract, with exact route-local role
  ordering, access, program ownership, and pairwise nonaliasing enforced by
  both SBF composers;
- hostile decoding of the current Realm/Profile/policy, MarketBinding/runtime,
  MarketInstance, Hoard V2, ClaimLedger V3, Position V3, and purpose-owned
  Replay accounts on full-vector custody routes, with current
  Hoard V2 and ClaimLedger V3 successors for the full-vector routes;
- projection through a named base-PDA verifier;
- projection through a named pinned Token-2022 parser; and
- exact ordered outer CPI/write plans plus hostile post-CPI reconciliation.

`CanonicalToken2022DecoderV1` is the concrete hostile-byte implementation of
the `Token2022DecoderV1` trust boundary. It uses the pinned Token-2022 layout and rejects
every mint extension, nonzero decimals, freeze authority, wrong mint authority,
or uninitialized mint. Holder accounts must reject frozen, native, delegated,
close-authority, wrong-mint state, and every extension except
`ImmutableOwner`. Runtime-contract projections are accepted only after that
parser returns them, and the runtime rechecks the fields it owns.

`wrapper_mint_parser_plan_v1` and `wrapper_token_parser_plan_v1` make those
requirements exact. `plan_token_2022_cpi_v1` emits the real Token-2022 byte
layouts and metas for `InitializeMint2`, `MintToChecked`, `BurnChecked`, and
`SetAuthority(MintTokens, None)`; it is not a mock execution result.

## Current successor ownership

The deleted canonical action-2/4 route previously manufactured a Structured
authority transcript for General V2 action 35. That parallel transfer planner,
its public preparation/reconstruction receipts, its 30-account wrapper
executor, and its duplicate General payload/CPI codec have been physically
removed. Family-envelope parsing may recognize the historical action-2/4 wire
tags only to refuse them deterministically; current Replay V3 construction,
encoding, and hostile decode do not admit either action.

Current actions project exact hostile-decoded Position V3 and purpose-Replay
V3 successors directly from the same Hoard V2, ClaimLedger V3, descriptor,
mint, holder, Product, collateral, and release observations used by their
private base compositions. No caller-shaped Market ledger, generic action-35
capability, or second transfer DTO can authorize those writes.

## Exact route staging

Every route calls the canonical runtime function first, on copies, before
returning an execution plan. The base CPI variants carry the returned runtime
plan itself rather than an adapter copy of its post-state.

| action | ordered outer operations |
| --- | --- |
| create | descriptor System allocation; mint System allocation; InitializeMint; descriptor write; base Product admission + Structured root admission + empty-vault creation |
| full wrap | base atomic full-vector custody + complete-set compression; MintToChecked |
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

The closable base Position/Replay pair and `0xb7/1` root are separate. The base
charges the Product-authenticated rent owner each complete current-bank
principal atop hostile prefunding, persists that principal separately from the
donation floor/residue, and freezes Product's neutral lamport sink. Later close
work may return only the persisted principal to its owner and must route every
donation to the sink. The wrapper cannot mint this authority from caller fields.

## Activation blockers

Every Structured tuple and the separate wrapper action mask remain disabled.
The single shared current account contract covers actions 1, 3, 5, 6, 7, and
8 with exact frame sizes 34, 32, 32, 32, 33, and 31 respectively. Actions 2
and 4 have no current executor, and current Replay V3 construction and hostile
decode refuse their allocated tags.

The source-staged routes include compaction, terminal redemption, Product
wrapper terminal promotion, canonical Position V3/Replay V3 retirement,
descriptor retirement, and Structured-root update or final close. They remain
`ReservedDisabled`; source presence is not activation evidence.

Activation requires replacement checked wrapper/base/Token-2022 release
artifacts whose exact manifest admits only the coherent current action set,
plus linked build, measurement, bank, SVM, local-validator, CPI-depth,
rollback, and rent evidence. None of that successor evidence has been run.
`SBF_EVIDENCE.md` records the explicit evidence state.

These are activation dependencies, not reasons to retain a duplicate planner
or to describe this family as live.
