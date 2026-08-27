# Counted retirement seam

`clutch-retirement` is an allocation-free `no_std` owner of retirement-specific
codecs and pure state-transition evidence. It does not allocate a live tag or
instruction, dispatch a request, perform CPI, mutate a Solana account, or
authorize deployment. Every current legacy runtime close remains fail-closed.

## Frozen and successor envelopes

The committed envelopes remain byte-for-byte frozen:

| Family | Composition | Exact bytes | Meaning |
| --- | --- | ---: | --- |
| Position V2 | 220-byte base + 60-byte tail | 280 | counted Position |
| general Epoch V5 | 329-byte base + 100-byte tail | 429 | nine counts, generation, rent split |
| Market V2 | 726-byte base + 8-byte cursor | 734 | monotone general-Epoch cursor |
| general Reservation V5 | 618-byte base + 9-byte count tail | 627 | frozen count-only schema |
| direct Reservation V6 | 618-byte base + 9-byte count tail | 627 | frozen count-only schema |

V5 and V6 do not own deletion funding and must never route Reservation
deletion. Their exact codecs and accepted byte sets remain available for
compatibility. Their frozen pure count transitions retain committed behavior,
but no live creation/deletion route may use them to authorize a deletable
Reservation.

The exhaustive 23-variant `RetirementErrorV1` and every frozen pure signature
retain their committed source behavior. Successor-only APIs use the distinct
`RetirementErrorV2`; the only cross-version conversion is lossless V1 to V2.
An exhaustive downstream compile fixture freezes the V1 variant set and proves
that historical exhaustive matches still compile. The declaration order is
also retained exactly, as checked against the committed source.

The deletable successors are distinct schemas:

| Family | Composition | Exact bytes | Status |
| --- | --- | ---: | --- |
| general Reservation V7 | 618 + 9 + 48 | 675 | codec-local, no SBF route |
| direct Reservation V8 | 618 + 9 + 48 | 675 | codec-local, no SBF route |

The 48-byte owner is exactly `payer[32] + refundable_principal:u64 +
donation_floor:u64`. It has no tombstone compartment. Admission always charges
the payer's full principal even if the target PDA was hostilely prefunded;
prefund becomes donation. Close refunds exact principal to the persisted payer,
routes the complete remaining balance to the Realm-selected neutral sink, and
leaves the Reservation absent. Direct V8 additionally requires an exact mirror
of the direct V2 base funding ledger until a clean central direct base successor
removes that compatibility duplication.

Position and general-Epoch tombstones have exact 76-byte and 84-byte codecs.
The central collision ledger reserves their `0x75/v1` and `0x76/v1`
coordinates as `ReservedDisabled`. This crate owns the exact local codecs, but
the reservation alone supplies neither live routing nor activation authority.

## Pure trust boundary

Types named `Adapter*ProjectionV1` and the projected live/account sibling
structs are public, forgeable DTOs. Pure transitions cross-bind their semantic
fields and reject aliases, wrong parents, wrong generations, wrong funding
targets, and scalar sink substitution. Those checks do not prove runtime owner,
PDA, executable, codec, balance, or account-byte authenticity.

`ValidatedAdmissionLedgerRetiredV1` is different: its fields are private and
its only constructor validates the complete pure `CandidateWindowV4` terminal
shape. It proves semantic Window structure only. There is no exact Window V4
account codec/PDA/owner/SBF adapter, so it is not runtime authentication and
cannot authorize root close.

The adapter now has one exact Direct Epoch V4 bridge: owner/PDA/header/length/
bump authentication, authoritative V4 decode, canonical checked
`epoch_index + 1`, projection of all six lifecycle phases, and projection of
the persisted neutral sink. Direct V8 registration requires pre-freeze-open;
frozen, selected, settled, and prefreeze-aborted parents refuse. No live route
uses that bridge. General neutral-sink provenance and Position/Replay
account/absence derivation remain explicit activation blockers.

## Atomic plans and enforced STOPs

The successor Position plan requires a separately funded, generation-scoped
Replay sibling. Position tombstoning and Replay deletion share one alias-safe,
coalesced credit plan. Reopen proves the prior Replay absence projection,
increments generation with checked arithmetic, and creates a new sequence-zero
Replay with its own full-principal admission. The frozen standalone Position
close/plan/reopen symbols retain their committed root-only pure behavior for
source compatibility, but they are not successor authorization and no live
route may call them. Only the V2 Position/Replay bundle models safe activation.

At the 84-byte reference Replay body, the deletion owner projects to 132 bytes.
The central collision ledger reserves `0x7a/v1` as `ReservedDisabled`, while an
in-flight external general-v2 contract proposes the 132-byte composition.
Retirement does not own its exact base codec or route it in SBF.

Epoch funding compartments are modeled as three disjoint principals: Epoch's
live/tombstone split, Window deletion funding, and Budget deletion funding.
Internal arithmetic coalesces repeated payer debits and close recipients before
any modeled write. It is deliberately non-executable evidence:

- `open_general_epoch_root` always returns
  `BudgetFundingUnauthenticated`; and
- `plan_epoch_root_retirement` always returns
  `BudgetRetirementUnauthenticated` after earlier malformed-input checks.

The authoritative Budget owner still needs an opaque funding/disposition
capability covering reward liabilities, cleanup markers, and every economic
compartment. Unpaid rewards are not surplus. The frozen standalone Epoch
open/close/plan symbols retain their committed root-only pure behavior for
source compatibility; no live route may treat them as complete root plans.

The frozen `LiveEpochV5` API retains its original three-state phase and accepts
the committed nonzero generation semantics. The successor
`LiveGeneralEpochProjectionV2` uses the exact five wire phases OPEN, FROZEN,
CLEARED, SETTLED, and LAPSED. Adapter promotion to that successor checks
`generation == epoch_index + 1`; the frozen V5 codec itself remains permissive.
Current general SBF does not stamp SETTLED, so terminal promotion remains
blocked.

The nine frozen Epoch count words are candidate bundle, CandidateIndex page,
candidate verdict, candidate escrow, ClearWork bundle, order page, Reservation
archive, settlement receipt, and final pot, in that order. Candidate admission
nodes are owned by `CandidateWindowV4`'s admission ledger, not a retrofitted
tenth V1 count.

Run:

```sh
cargo test --manifest-path crates/clutch-retirement/Cargo.toml
cargo test --release --manifest-path crates/clutch-retirement/Cargo.toml
cargo clippy --manifest-path crates/clutch-retirement/Cargo.toml \
  --all-targets -- -D warnings
cargo doc --manifest-path crates/clutch-retirement/Cargo.toml --no-deps
```
