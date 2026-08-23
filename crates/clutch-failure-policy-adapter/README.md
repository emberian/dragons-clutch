# Failure-policy account adapter contract

Status: **STANDALONE PURE ADAPTER CONTRACT / NOT CENTRAL-REGISTRY ALLOCATED OR ROUTED**.

This crate gives the funded successor failure runtime an exact durable root
codec, a distinct program-owned reserve authentication boundary, a fixed
intent preimage, and atomic mutation projections. It does not allocate a live
central instruction tag or account tag, derive a Solana PDA, parse Clock,
invoke System/Token programs, or authorize a deployment.

The durable root and expendable reserve are deliberately different accounts.
The root persists the complete canonical `FailureRuntimeV1` state. The reserve
key must equal the runtime's immutable `recovery_state_id`; its lamports are
the exact balance consumed by transition planning. Root rent is a market-core
obligation. Recovery work/rent principal remains solely inside the reserve
ledger and never becomes root rent, Hoard principal, fee revenue, or treasury
funding.

Authentication checks exact expected root key, owner, writability, reserve
key, reserve owner, reserve zero-data shape, and non-aliasing. It confers no
signer authority. Every transition rewrites the whole canonical root, checks
the plan against the decoded prestate, exposes exact pre/post reserve balances
and four transfer compartments, and requires the external adapter to perform
and verify those movements atomically before storing the new bytes.

Initialization is a separate one-shot projection. It accepts only an exact-size
all-zero durable root and zero-data reserve owned by the live program, then
matches the full private admission receipt to the runtime binding, V5 Series,
ordinal, V2 occurrence, FundingQuote, recovery state/generation, both initial
principal compartments, and the observed reserve balance. Durable root rent is
persisted with its immutable payer and exact adapter-authenticated principal,
is preserved independently, and is never counted as recovery capital. Hostile
root prefund or later unsolicited lamports cannot become rent or principal.

The fixed intent preimage binds action, immutable failure-policy binding,
full-width V2 market identity, generation, expected replay nonce, Clock,
Window/evidence/work/terminal receipt identities, the authenticated recovery
quote schedule and exact per-call ceiling for paid work, progress, and refusal
code. Inactive action fields must be zero. Submission identity, resolver
identity, caller-selected payout, and arbitrary transfer destinations are
absent.

Action-specific pure projections consume the existing runtime's private-field
accepted-resolution, liveness-work, and terminal capabilities. They bind every
authenticated artifact back to the fixed intent before producing the exact
root/reserve mutation. Source and relation refusal projections likewise pass
the complete SourcePlane objects through the semantic runtime; an intent alone
can never establish a refusal or accepted value.

Terminal projection is acyclic. A resolved or dormant failure runtime emits a
recovery-funding close receipt for the liveness Recovery compartment; dormancy
means only that the finite funded campaign ended. A distinct full lifecycle
join is emitted only after resolution plus authenticated retirement root,
permanent replay tombstone, and final SourcePlane release. Neither receipt
consumes a liveness terminal receipt as an input; its own typed ID is the
family-specific receipt projected into liveness.

After that full lifecycle join, root close refunds only the persisted root-rent
principal to its payer and sends every excess root lamport to the immutable
neutral sink. The expendable reserve must already be zero. The replay tombstone
is a separately owned permanent fact and is never closed by this adapter.
