# dclutch-claims-svm

This standalone crate is the capability-neutral physical Claims child ABI.
Callers retain their own candidate, order, ticket, and settlement semantics and
commit those facts into `request_id`. Claims receives only the exact economic
basket that it owns: one Market, zero to two dynamic Position owners, optimistic
Claims revisions, and a runtime-width vector of exact claim atoms.

The packet deliberately has no `[u64; 16]`, no family-specific context union,
and no duplicated collateral or token state. A Registry-authenticated caller
signs for the packet digest; the Claims program returns a receipt binding that
digest and exact post-revisions. Core Split/Redeem uses the separately shared
Market-Core effect packet and is not re-encoded here.
