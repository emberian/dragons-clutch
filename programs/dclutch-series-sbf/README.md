# dClutch Series SBF adapter

This standalone program refines the Lean-owned Series interpreter into an
authenticated Solana account and canonical Market-Core CPI boundary. It is not
yet registered in the root workspace or release manifest.

Routes are distinguished by exact Lean-owned instruction widths:

- 552 bytes: bootstrap `Template || Series || Ticket`;
- 216 bytes: precreate and predeposit the next Ticket's resources;
- 64 bytes: consume, expire, or close using `RequestV1`.

Every route authenticates current Rent and Clock and invokes Registry to
reauthenticate the selected Core program against its current ProgramData
deployment. Claims and Custody authority is not borrowed by Series: selected
Core owns those downstream calls and reauthenticates their current programs.

Template, Series, and Ticket use separate PDA domains. Bootstrap accepts a
System-owned, zero-data PDA even when an unsolicited transfer pre-funded it,
tops up only the shortfall, then allocates and assigns it by PDA signature.
Observed balances are floors: surplus lamports remain an unclassified donation
and are never reinterpreted as rent, work funding, close funding, or principal.
The separately named Series close-rent amount is the only surplus the terminal
route may transfer. The canonical 336-byte `SeriesCoreRequestV1` carries four disjoint Ticket funding
facts: Hoard principal, Market rent, capability rent, and work funding. A
Prepare call makes selected Core responsible for precreating/predepositing
those exact resources; the Series-owned Ticket persists the economic record and
replay revision without duplicating the physical balance.

Series sends the canonical 336-byte request directly to selected Core. The
Series program signs only `SeriesCoreCallerSeedsV1`, whose PDA commits the
Template and SHA-256 digest of all request bytes. It accepts only the selected
Core program's 264-byte `SeriesCoreAckV1`, which echoes the action, release,
Template, optional Ticket/Market, request digest, optimistic revisions, and
nonzero occurrence-derived Market generation, and binds Core's post-resource
digest. The generic `CoreEffectEnvelopeV1` is intentionally absent here: it is
for Core's downstream release-role effects, not for treating Series as a role.

Execution candidate bytes are written only after Core succeeds and returns that
exact acknowledgment. Bootstrap and Ticket funding write their validated
prestate before CPI so Core can authenticate the newly created PDAs; a failing
Core call rolls those writes and account creation back with the transaction.
Terminal close rent moves only after Core succeeds. Host tests prove that an
execution-stage failure exposes no candidate bytes to the commit boundary.

## Exact Series prefix and Core tail

1. signer/payer or permissionless worker
2. Template PDA
3. Series PDA
4. Ticket PDA
5. Registry/Core Program
6. activated release-set cache
7. current Core Program
8. Core ProgramData
9. System Program
10. Rent sysvar
11. Clock sysvar
12. precommitted Market/staging account
13. exact work/refund beneficiary
14. canonical Core-caller authority PDA
15+. opaque action-specific Core-owned account tail

Template is writable only during bootstrap. Series is read-only during
next-Ticket funding. Series and Ticket are writable during execution. Series
does not reinterpret or freeze the action-specific Core tail; it forwards the
caller authority plus tail unchanged, and selected Core is responsible for its
own exact frame and every downstream Claims/Custody account.

## Current integration boundary

The private Series role envelope has been deleted. This adapter consumes only
the Lean-owned Series ABI, canonical Market-Core physical ABI, and current
Registry receipt. The action-specific Core SBF account tail and Core's canonical
Claims/Custody dispatch are not yet implemented, so a real-SVM end-to-end
campaign cannot honestly run yet. Until those land, this is host and optimized
verifier build evidence—not executable lifecycle evidence.

Standalone checks:

```sh
cargo test --manifest-path programs/dclutch-series-sbf/Cargo.toml --offline
cargo clippy --manifest-path programs/dclutch-series-sbf/Cargo.toml \
  --all-targets --offline -- -D warnings
cargo build-sbf --manifest-path programs/dclutch-series-sbf/Cargo.toml \
  --lto --optimize-size
```
