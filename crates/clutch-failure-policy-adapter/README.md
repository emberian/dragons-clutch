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

The fixed intent preimage binds action, immutable failure-policy binding,
full-width V2 market identity, generation, expected replay nonce, Clock,
Window/evidence/work/terminal receipt identities, the authenticated recovery
quote schedule and exact per-call ceiling for paid work, progress, and refusal
code. Inactive action fields must be zero. Submission identity, resolver
identity, caller-selected payout, and arbitrary transfer destinations are
absent.
