# Decision 0004: the founding capability root is derived, not persisted

Status: accepted on 2026-08-26 as the resolution of Blocker A in
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`. This is an
authority and wire decision, not release or deployment evidence. It does not
change what a capability root *is* after a Market is Open, it does not change
the ordinary activation route that creates one, and it is not a claim that the
atomic founding outer is reachable — Blocker B remains open and is not
addressed here.

## Context

The atomic generic founding outer (`DCLTGMF1`,
`programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs`) runs
Lock → Found/permit → Realize → Claims FoundingV5 → Core Open in one rollback
domain. Core's Found stage authenticates a Trading capability root before it
creates the Market:

```text
generic_founding_v1.rs:481   authenticate_root(frame.capability_root, ...)
generic_founding_v1.rs:686   root.owner   == trading_program
generic_founding_v1.rs:687   root.data_len() >= CAPABILITY_ROOT_HEADER_BYTES_V1
generic_founding_v1.rs:708   header.market() == request.market()
generic_founding_v1.rs:755   frame.found.market.owner    == system_program
generic_founding_v1.rs:756   frame.found.market.data_len() == 0
found.rs:310                 (independently, the same vacancy requirement)
```

So the root account must already be Trading-owned and allocated at the moment
the Market account must still be vacant. The only in-tree route that creates a
`CapabilityRootHeaderV1`-headed root is
`programs/dclutch-trading-sbf/src/outer.rs::process_activation`, whose
`authenticate_market_and_caller` (`outer.rs:409`) requires
`market.owner == core_program && market.data_len() == STATE_BYTES`, and whose
upstream Core transition `activate_capability_child`
(`crates/dclutch-market-core-codec/src/generated.rs:1063`) requires
`state.phase == Phase::Open`. `Phase::Open` is set only by `open_series_market`
(`generated.rs:945`) — the Open stage of this same founding.

```text
capability root <= activation <= Phase::Open Market <= Core Found <= capability root
```

`386f254` resolved a *hash* cycle in the same conjunction (the selected config
identity may not commit back to the root address; `selection_preimage()` owns
the sole root-free preimage). This is a separate *lifecycle* cycle and that fix
does not touch it.

Two further facts decide the shape of the resolution.

**The root account is never dereferenced anywhere else.** Across the whole
founding path, `authenticate_root` is the only reader of the root's *bytes*.
Every other use is of the root *address*: it is seed three of
`ProjectedCustodyCallerSeedsV1`
(`crates/dclutch-custody-contract/src/projected.rs:178`), which authorizes the
Trading signer for Lock and Realize; it is echoed by
`authenticate_generic_projected` (`generic_founding_v1.rs:943`), by
`FoundingIntentV5.parent_root`, and by `authenticate_open_request`
(`generic_founding_v1.rs:1250`). `Pubkey::find_program_address` needs no
account to exist.

**Reading the root proves nothing the request does not already fix, and it
leaves a real hole.** Every field of `CapabilityRootHeaderV1` is a pure
function of the founding request plus the Market-selected capability manifest:
`release_set`, `market`, and `generation` come straight from the request, and
`selection.config` is `hash(request.selection_preimage())`. The remaining four
— `manifest`, `entry_index`, `kind`, `capability_release` — are today supplied
entirely by the caller (`construct_generic_founding_root_selection_v1`,
`crates/dclutch-market-founding-v1-operator/src/lib.rs:108`) and are checked
only for self-consistency: the header must re-derive to the account's own
address. **Nothing at founding binds the selection to the Market's
authenticated capability manifest.** A founder may name any manifest, any entry
index, any kind, and any capability release, and Found accepts it.

`docs/decisions/0003-fixed-role-capability-execution.md` already fixed the
principle for the ordinary route: "Core rebuilds the prefix from the
hostile-decoded Market-selected manifest and the exact indexed entry. It does
not accept any Program coordinate from the request." Founding does not.

## Decision

**The founding capability root is derived by Core and never read, and no root
account exists during founding.**

Core reconstructs `CapabilityRootHeaderV1` from facts it has already
authenticated, derives the root address under the Trading program, and requires
`request.capability_root()` to equal it. The capability-root *account* is
created afterwards, by the unchanged ordinary activation route
(`outer.rs::process_activation`), against the now-Open Market, in its own later
transaction.

```text
market, generation, release_set          <- the founding request
selection.config                         <- hash(request.selection_preimage())
selection.manifest                       <- authenticated Market identity
selection.entry_index                    <- request.capability_entry_index()
selection.kind, selection.release        <- that entry of the authenticated
                                            capability-manifest record
capability_root = find_program_address(header.seeds(), trading_program)
require request.capability_root() == capability_root
```

Trading remains the sole creator and sole writer of capability roots. Core
creates none, signs nothing new, and gains no new PDA domain. There is no
"founding root" concept, no pre-Market activation route, and no second
lifecycle for one account.

### Why this and not the alternatives

This is the only candidate that is *strictly stronger* than the status quo. It
adds the manifest-entry binding that founding does not have today, and it
removes an account whose contents were already implied. It is also the most
conservative: no new authority, no new account, no new wire family, no new
stage inside the Open-last rollback domain.

The atomic outer's rollback domain is unchanged: founding still commits Open
last, and capability activation stays outside it. Activating a capability is
ordinary post-founding work with its own frame and its own compute budget;
folding it in as a sixth stage would enlarge a transaction that is already 139
account references and five CPIs, and would couple Market creation to
capability admission for no authority gain.

## Exact wire and authority consequences

`GENERIC_FOUNDING_REQUEST_BYTES_V1` stays **400**. Bytes `392..400` are
currently written as zero by `encode` and neither read nor canonicality-checked
by `decode`. They gain meaning:

| Offset | Width | Field |
| ---: | ---: | --- |
| 392 | 2 | `capability_entry_index`, little-endian |
| 394 | 6 | reserved, must be zero |

`decode` must reject nonzero reserved bytes at `394..400`, which closes an
existing canonicality gap. The entry index is not independently bounded; the
authenticated manifest's actual entry count is the sole bound, exactly as in
Decision 0003.

Because `selection_preimage()` re-encodes the whole request, every request's
selected config identity — and therefore every derived capability-root address
— changes. Nothing is deployed, so this is a fixture regeneration, not a
migration.

Account-frame consequences:

| Frame | Before | After |
| --- | ---: | ---: |
| Core Found (`GenericFoundAccounts`, index 33) | root account present | removed |
| Core Open (`GenericOpenFrame`, index 15) | root account present | removed |
| `GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1` | 35 | 34 |
| `GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1` | 24 | 23 |
| `DCLTGMF1` outer at `funding_count = 3` | 139 | 137 |

The Found frame must instead carry the Market-selected capability-manifest raw
record and its vacant staging cursor if it does not already; the manifest
content identity is authenticated as part of the Market identity, so the record
is authenticated by the existing `authenticate_finalized_record` path and no
new record authority appears.

Authority consequences:

- Core gains one requirement it did not have: the selection must name the
  Market's own authenticated capability manifest and one entry that manifest
  actually contains, with that entry's exact `kind_id` and `release_id`.
- Core loses one requirement: the root account need not exist. It never proved
  anything, because its contents are a function of the request.
- `ProjectedCustodyCallerSeedsV1` is unchanged. Its `parent_capability_root`
  seed is the same derived address as before, now derived rather than read, so
  the Lock and Realize signers are unchanged in kind and different only in
  value (because the config digest moved).
- `outer.rs::process_activation` and `authenticate_vacant_root` are unchanged.
  A Market founded under this decision activates its capabilities exactly as
  any other Market does, and the address the founding pre-committed is the
  address activation will later create.

## Required refusals

The converged implementation must include adversarial coverage for:

- a substituted `capability_root` coordinate that is not the derived address;
- a selection naming a capability manifest other than the Market's own;
- an `entry_index` outside the authenticated manifest's actual entry count;
- a substituted `kind_id` or `capability_release` at a valid index, including
  one taken from a different entry of the same manifest;
- nonzero reserved bytes at `394..400`, and a request whose only difference is
  those bytes;
- a founding whose Open-stage derivation disagrees with its Found-stage
  derivation, which must be impossible and must be pinned by a test rather than
  asserted;
- a replayed founding request whose derived root address collides with an
  already-activated root for the same Market and generation;
- a `ProjectedCustodyRequestV1` whose `parent_capability_root` is any address
  other than the derived one, at both Lock and Realize; and
- any refusal after an earlier write or CPI, with transaction-wide rollback
  checked byte-for-byte.

The existing `request_join_*` tests do not substitute the capability root at
all. They must gain that case.

## Convergence file plan

1. `crates/dclutch-market-core-codec/src/generic_founding_v1.rs` — add
   `capability_entry_index()` at offset 392, reject nonzero `394..400`, and
   extend `selection_preimage_is_root_free_stage_free_and_coordinate_bound`.
2. `programs/dclutch-core-sbf/src/generic_founding_v1.rs` — replace
   `authenticate_root` with a derivation that takes the authenticated manifest
   entry, and use it at both `:481` (Found) and `:559` (Open). Drop the root
   account from `GenericFoundAccounts` and `GenericOpenFrame`; update
   `:1557` to compare against the derived address.
3. `crates/dclutch-market-core-codec/src/generic_founding_v1.rs` — reduce
   `GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1` to 34 and
   `GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1` to 23, keeping the
   `FOUND_ACCOUNT_COUNT_V2` static assertion honest.
4. `programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs` — follow
   the frame arithmetic (137 at `funding_count = 3`) and add the missing
   capability-root substitution case to `request_join_*`.
5. `crates/dclutch-market-founding-v1-operator/src/lib.rs` — stop accepting
   `manifest`, `kind`, and `capability_release` as free parameters; take them
   from the authenticated manifest entry the caller names by index, and keep
   `capability_root_selection_is_acyclic_and_satisfies_core_authentication`
   evaluating exactly the conjunction Core evaluates.
6. `tools/local-validator/bootstrap/successor` — regenerate the demo Market's
   selection and root address; the manifest it already publishes becomes the
   authority for kind and capability release.
7. Delete nothing else. `outer.rs::process_activation`,
   `authenticate_vacant_root`, `CapabilityRootSeedsV1`, and
   `CapabilityRootHeaderV1` are unchanged and remain the sole capability-root
   creation authority.

## Rejected alternatives

**Reorder root creation into the atomic outer after the Found stage.** The root
is authenticated at Found, before `found::apply_prepared` creates the Market, so
"after Found" is already too late for Found's own conjunction. Making this work
requires *two* separate weakenings: dropping `authenticate_root` from the Found
stage, and widening `activate_capability_child` to admit a Founding-phase
Market. The second is a permanent enlargement of the capability-activation
membrane — every family's activation would thereafter be legal against a Market
that is not Open — bought to fix one transaction's ordering. Rejected.

**A pre-Market founding root committing to the derived future Market address.**
The Market address is genuinely derivable before the account exists
(`MarketCoreStateSeedsV2::new` omits `market_id`;
`crates/dclutch-market-core-codec/src/physical.rs:634`), so this is
*satisfiable*. It is not *warranted*. It adds a new Trading signing surface
with its own adversarial budget, a second lifecycle for one account
(create-at-founding then upgrade-or-confirm-at-Found, i.e. two truths about one
root over its life), and an abort path for roots whose founding never
completes — all to persist an account that nothing dereferences and whose every
field is already derivable. The evidence document guessed this was the likelier
answer; the ground truth that the root is never read undercuts its motivation.
Rejected.

**Keep the root account but relax `header.market() == request.market()`,
keying the founding root on the sponsoring context instead.** This drops the
root's single-use binding to the exact Market being founded, which is the one
property that makes a capability root safe to reuse as a Custody namespace
seed. It weakens a refusal to make a route reachable. Rejected.
