# Series recurring request parameterization

Owner: correctness_architecture. Implementation in progress; no selected SBF
or validator acceptance is implied.

The five-action ProgramSet selects only on the action byte. Its current Prepare,
Consume and Expire Effects nevertheless embed occurrence-specific child request
bytes. Changing the occurrence therefore changes an immutable selected artifact.
Parent-root normalization does not solve this conflict.

The executable native diagnosis is
`series_found_prepare_campaign::recurrence_tests::known_defect_same_bundle_second_prepare_retains_first_occurrence_market`
in `tools/local-validator/bootstrap/successor/src/series_recurrence_tests.rs`.
It compiles the real five-action source from the ordinary Market publisher,
advances Series replay with native Prepare/settle/Retire transitions, derives the
second occurrence through the native child codecs, and executes the original
RequestProfile, TransitionVM and Effect on that second family request. Its
positive control requires exact equality with the first native Initialize
request. Its known-defect result requires the first Market and Ticket-derived
context to remain in the second projected request. The Effect scratch accounts
are component fixtures, not physical-authentication or transaction evidence.

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
| Consume Lock/Realize receipts, permit and Claims candidate | `series_founding_children_v1`; native Custody/Core/Claims receipt codecs | Observed post-Prepare projected state/replay; canonical predicted Core geometry; actual child receipts remain runtime authority |
| Complete per-occurrence child bank | `series_child_bank_v1` | Its native content/physical joins plus authenticated replay and current family request, not host assertions |

The current child-bank and founding-child combinators live in `dclutch-operator`
and depend on Trading's SBF adapters. Runtime reuse must move their semantic
implementation to the existing shared adapter layer and make the operator
consume it; adding an SBF dependency on the operator would create a cycle.

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

## Required executable closure

Retain one exact ProgramSet, descriptor bank, certificate and selected include
through both occurrences. Derive both branches with native codecs and replay:
one occurrence consumes, the other expires, then both Ticket accounts retire
and the root closes. The projected child bytes must equal those native requests;
substituted occurrence/Ticket/root state and stale derived bank must refuse
specifically. Only then build the selected accelerator and measure actual
account creation, child permissions, CU/heap and terminal poststates on the
local validator. No width increase or fixture substitution closes this defect.
