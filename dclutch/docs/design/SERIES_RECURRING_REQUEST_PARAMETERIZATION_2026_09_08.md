# Series recurring request parameterization

Owner: correctness_architecture. Implementation in progress; no selected SBF
or validator acceptance is implied.

The five-action ProgramSet selects only on the action byte. Prepare, Consume
and Expire now publish zero native-request templates and materialize every
request word from private, family-and-root-bound adapter banks. Prepare also
writes its native root successor: the prior artifact created a Ticket without
persisting the root's prepared flag, outstanding count, or revision.

The executable native control is
`series_found_prepare_campaign::recurrence_tests::same_bundle_native_consume_then_expire_preserves_all_replay_poststates`
in `tools/local-validator/bootstrap/successor/src/series_recurrence_tests.rs`.
It keeps the exact occurrence-zero ProgramSet through both occurrences; an
independent occurrence-one compilation must produce identical five-action
publication bytes. It uses the ordinary Market publisher's same retained Plan,
actual occurrence codecs, and native replay transitions. It compares complete
Prepare and Expire root/Ticket data projections with native encoded successors,
preserving the canonical immutable root header; materialized Consume requests
execute native projected-Custody Lock and Realize. Both Tickets remain until
the root is terminal, then native Retire families execute the retained Retire
artifact before native Close selection. Stale family/root banks refuse without
changing their scalar destination.

This is component evidence. Its account-profile register inputs and account
memory are explicit scratch projections, and its terminal Rent observations are
component fixtures. It does not authenticate AccountInfo provenance, execute
Core/Claims CPIs or token transfers, realize FundingV5 account creation/deletion,
or prove atomic transaction rollback. Runtime seeding/authentication, selected
SBF frame/heap/CU measurement, and local-validator poststates remain required.
The earlier executable recurrence falsifier is preserved at `cbc66cf9a`: its
retained Prepare emitted occurrence zero's Market and Ticket context for the
second family request. That milestone proved child projection only.

## One semantic construction owner

Keep the current five-action selector and native child semantics. An
adapter-owned, private per-occurrence fact bank must validate the selected
Template, occurrence proof, Ticket and root replay before producing runtime
values for the generic Effect writer. Host, Trading and Shadow must call the
same construction owner. A caller-provided digest or copied request bank is not
an authenticated fact bank.

The existing construction owners to reuse are:

| Facts | Current owner | Required runtime evidence |
| --- | --- | --- |
| Occurrence, Ticket, schedule, principal, replay revision | `dclutch-trading::series::{request,escrow,replay}` and `series::operator` | Selected Template; Merkle-admitted occurrence; matching Ticket; decoded root/Ticket state; Clock |
| Normal Custody Prepare/Expire requests and their parent commitment | `series::custody_v3`; `derive_series_prepare_parents_v1` | Actual family request hashed by the adapter; selected programs and authenticated token/account observations |
| Projected Custody context and requests | `series::projected_custody_v3` | Domain-separated Ticket context; derived future Market; canonical physical identities and Rent |
| Core ProjectFound receipt digest | Core `ProjectFoundReceiptV2`; Custody `projected::process_initialize` | Custody already invokes Core and checks producer, receipt bytes and exact Found-request digest; retain this owner |
| Consume Lock/Realize receipts, permit and Claims candidate | `series::founding_children_v1`; native Custody/Core/Claims receipt codecs | Observed post-Prepare projected state/replay; canonical predicted Core geometry; actual child receipts remain runtime authority |
| Complete per-occurrence child bank | `series::child_bank_v1` | Its native content/physical joins plus authenticated replay and current family request, not host assertions |

The child-bank and founding-child implementations now live in the Trading
adapter; the operator modules re-export them. `derived_prepare_v1` and
`derived_terminal_v1` bind native construction to the actual family, root and
replay snapshot. Core's canonical expected ProjectFound receipt constructor is
shared with Trading. No request-bank input supplies a future receipt or digest.
The subsequent Custody CPI still verifies the actual Core producer and receipt.

Matched Claims requests and Core permits now have one pure construction owner,
`dclutch-claims::founding_plan_v1`. Core supplies authenticated physical facts
and retains admission and the actual permit write; Trading predicts the pair
from its authenticated occurrence bank. The owner derives the positive exact
principal/Product-payout-scale quotient. Both the intent and Claims request
name the projected State address that Realize rewrites in place, and Claims
binds the fresh normal replay's 0-to-1 transition. Lock's source replay closes
and is a separate address. The adapter also applies the native Source cap and
joins it to the predicted Core state. Market publication previews expose the
same compiler-derived cap so component fixtures cannot invent another ceiling.
Actual Core-written permit bytes remain an independent integration obligation;
agreement between two callers of this constructor alone does not prove their
account observations were authenticated correctly.

## Projection and immutable artifacts

AccountProfile already provides `ProjectKey`, `ProjectDataIdentity`,
`ProjectDataU64` and related integer projections. Effect already provides
`write_request_identity` and `write_request_u8/u16/u32/u64`. Use those existing
operations for the final request materialization. Keep hashing outside the
generic VM: `ProjectDataDigest` only projects a digest the adapter established;
it is not a hash instruction for reconstructed request bytes.

The artifact emitter must leave occurrence-specific fields to this proven
runtime bank. The selected host acquisition must likewise stop treating the
first child Market as immutable `SeriesOccurrenceAuthorityV5`. Its current
Market/generation must come from the same admitted occurrence used by Trading
and Shadow. Prepare, Consume and Expire converge together. Retire and Close
must be checked for retained occurrence facts before claiming full ProgramSet
invariance.

The Template's occurrences may also differ in physical data widths or funding
counts. The profile must state which geometry is common, which is dynamically
selected, and which variation is refused by the Template's declared profile.
First-occurrence observations cannot silently establish all-occurrence geometry.

The current selected compiler has a **provisional geometry compatibility
constraint**: every occurrence must fit the exact fixed widths observed for the
first publication. Portfolio coefficient tails and the declared FundingState
span use their existing dynamic geometry; nested Basis/Source record widths do
not. In particular, a differently sized Basis or a change between absent and
present manipulation-Floor evidence is not established executable by this
compiler. The runtime AccountProfile refuses incompatible observations before
bank seeding. The compatible two-occurrence proof is not a publication-wide
compatibility certificate.

The lifting plan is concrete: after the first actual selected evaluator/Effect/
child-prefix resource probe, extend publication admission to consume every
occurrence's canonical future-Market closure and compare its owner-derived
profile geometry with the selected profile. Reject a differing shape before
publication, or use an existing dynamic span where its semantic owner supports
one. Until that admission exists, full Series completion and heterogeneous
Template support remain unclaimed.


## Required executable closure

Retain one exact ProgramSet, descriptor bank, certificate and selected include
through both occurrences. Derive both branches with native codecs and replay:
one occurrence consumes, the other expires, then both Ticket accounts retire
and the root closes. The projected child bytes must equal those native requests;
substituted occurrence/Ticket/root state and stale derived bank must refuse
specifically. Only then build the selected accelerator and measure actual
account creation, child permissions, CU/heap and terminal poststates on the
local validator. No width increase or fixture substitution closes this defect.
