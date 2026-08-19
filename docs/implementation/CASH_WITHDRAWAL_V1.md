# Pooled free-cash withdrawal

Status: **SBF-EXECUTED in a local in-process Agave bank**, 2026-08-19.
This is evidence for the owner cash-exit boundary only. It is not a deployment,
audit, public-cluster result, or a claim that order reservation and settlement
are complete.

## 1. Economic transition

`PositionAccount::cash_atoms` is total position cash and
`reserved_cash_atoms` is its encumbered subset. Therefore:

```text
free = cash_atoms - reserved_cash_atoms
require amount > 0
require amount <= free

Position.cash_atoms'          = Position.cash_atoms - amount
Position.reserved_cash_atoms' = Position.reserved_cash_atoms
Hoard.collateral_atoms'       = Hoard.collateral_atoms
Hoard token amount'           = Hoard token amount - amount
owner destination amount'     = owner destination amount + amount
```

The transfer is a Token-2022 `TransferChecked` signed by the canonical Hoard
authority PDA. The owner signs the instruction and binds the exact destination
address in `Intent::WithdrawCash`. The program separately decodes that
destination's current mint, owner authority, state, and extensions.

The instruction can run after market resolution and while a Position is in
close-requested state. A fully closed Position refuses. Replay advances by
exactly one on success.

## 2. Account plane

The account list is exact and contains twelve distinct roles:

| index | role | access |
| ---: | --- | --- |
| 0 | Position owner | signer |
| 1 | Market | program-owned, read-only |
| 2 | Hoard accounting | program-owned, read-only |
| 3 | Position | program-owned, writable |
| 4 | owner-generation Replay | program-owned, writable |
| 5 | frozen Profile | program-owned, read-only |
| 6 | authenticated collateral-policy bytes | read-only |
| 7 | pinned Token-2022 program | executable, read-only |
| 8 | Realm collateral mint | read-only |
| 9 | owner collateral destination | writable |
| 10 | derived Hoard authority | read-only, data-empty |
| 11 | derived Hoard collateral account | writable |

Market, Hoard, Position, Replay, Profile, authority, and Hoard-token PDAs are
rederived. Stored bumps and every cross-account identity edge are checked. The
collateral policy is recomputed against the frozen Profile, and the mint and
both token accounts are admitted under that policy.

## 3. Ordering and rollback

All program-state post-values are computed before the CPI. The Hoard and owner
token balances are snapshotted, and the instruction precomputes both exact
post-balances with checked arithmetic. It refuses before transfer if the
resulting Hoard balance would fall below locked claim backing.

After the CPI, both token deltas must be exact and the Hoard must still satisfy:

```text
actual Hoard token amount >= HoardAccount.collateral_atoms
```

Only then are Position and Replay bytes written. Any token failure, unexpected
delta, failed postcondition, or later transaction instruction failure relies on
Solana transaction atomicity to restore both token and program accounts.

## 4. Surplus and multi-owner meaning

An unsolicited direct transfer into the Hoard creates unowned surplus. It does
not credit a Position, increase a withdrawal allowance, become a fee, or create
a sweep right. A withdrawal amount is bounded only by the signer's own
unreserved Position cash; the program never spends another Position's cash,
locked claim backing, or an unowned donation.

The locally checkable coverage floor cannot enumerate all Positions. The full
market relation remains inductive:

```text
H = L + sum(position cash) + direct-deposit surplus
```

where reserved cash is a subset of position cash, not an additional term.
Exact Endow and Withdraw deltas preserve that relation. A future aggregate cash
account would be a second semantic owner unless the persisted layout and every
transition are deliberately migrated to make it the single owner.

## 5. Evidence and open boundary

The production wire codec and hostile forms pass the layout crate's host suite.
The handler's typed transition tests cover exact debit, reservation preservation,
post-resolution liveness, signer/binding/refusal order, and replay.

The real Token-2022 in-process bank covers:

1. refusal of one atom above free cash with all watched bytes unchanged;
2. withdrawal of the exact free amount while reserved cash and locked backing
   remain unchanged;
3. two identical withdrawals in one transaction, where the first transfer
   executes, the second refuses replay, and the runtime restores all four
   watched accounts;
4. destination substitution and a collateral account controlled by another
   authority; and
5. an unsolicited Hoard donation plus two Positions independently withdrawing,
   leaving the donation, the first Position's reservation, and locked backing
   in custody.

The focused run passed 16/16 collateral-plane cases and 6/6 bearer-token cases.
The compiled ELF was 549,560 bytes with SHA-256
`23139487e1a38de73a7f0077fb87cc28a1f1968a9dc8db0e2f5babcd09ebce41`;
the exact maximum-free withdrawal consumed 229,773 compute units. This digest
was built while the shared integration tree also contained an in-flight
resolution-replay repair, so it is focused runtime evidence rather than a clean
baseline artifact. The clean joined evidence ladder must rebuild and supersede
it.

Open reservation discipline remains a release gate. `WithdrawCash` itself
honors `reserved_cash_atoms`; every other cash consumer, order cancellation,
settlement, and close transition must preserve the same subset invariant. Until
that joined campaign is green, this instruction closes the direct cash-exit
surface but does not promote the full funded venue lifecycle.
