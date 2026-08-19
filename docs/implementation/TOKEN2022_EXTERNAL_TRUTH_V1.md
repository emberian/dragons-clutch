# Token-2022 external truth and bearer exit

Status: **IMPLEMENTED HOST CUTOVER; SVM EVIDENCE OPEN**, 2026-08-19.  The SBF
adapter uses canonical Token-2022 mint supply for every claim transition and
implements positionless `RedeemExternal`.  The old owner-shadow differentials
are retained disabled as migration archaeology.  Replacement host
differentials and the real Token-2022 committed-bank promotion campaign remain
release stops.

This design is deliberately narrower than a new token standard.  Outcome Eggs
remain ordinary transferable, extension-free Token-2022 tokens.  The protocol
does not scan holders, intercept transfers, prohibit ordinary burns, or depend
on an indexer.

## 1. One semantic owner for each fact

For every active outcome `i`:

```text
I_i = SupplyLedger.internal_supply[i]
E_i = actual Token-2022 outcome-mint supply[i]
T_i = I_i + E_i
```

`I_i` is an inductively maintained aggregate over program-owned Position
balances.  `E_i` is read from the authenticated outcome mint in the current
transaction.  `T_i` is rederived from those two terms before it is used by the
kernel.  No owner-local program account represents external holdings.

The holder's presented Token-2022 account is the only balance authority for a
bearer operation.  Its mint, owner authority, amount, state, and admitted
extensions are decoded from Token-2022 bytes in the current transaction.

`ExternalAccount` leaves every production account list and is not founded for
a new market or position.  Its codec may remain temporarily in the offline
reference harness so old vectors can be read, but production code must neither
derive it nor authorize, debit, credit, or reconcile against it.

`SupplyLedger.external_supply` may survive the first ABI cutover only as a
**last-observed mint-supply cache**, not as balance authority.  On every
production transition that can observe or change claims, all outcome mints are
presented in canonical outcome-index order and the following admission runs:

1. Require the pre-sync ledger and kernel to close:
   `kernel.total_supply[i] == I_i + cached_E_i`.
2. Authenticate every outcome mint and read `actual_E_i`.
3. Require `actual_E_i <= cached_E_i`.  Only the market PDA can mint, so an
   unexplained increase is corruption or an unaccounted program mint and must
   refuse.  A decrease is an ordinary holder burn.
4. Set the in-memory cache to `actual_E_i` and rederive
   `kernel.total_supply[i] = I_i + actual_E_i` with checked arithmetic.
5. Run the requested kernel and token transition.
6. Re-read all mints.  Require exact zero supply delta on untouched mints and
   the exact requested delta on the one touched mint.  Persist the resulting
   observed cache and derived kernel total.

Thus the cache can detect an impossible increase and document the amount of
newly observed forfeiture, but it can never override current Token-2022 state.
A later account-layout version should rename the field to
`observed_mint_supply` or remove it.

All outcome mints are carried because synchronizing only the touched mint is
not live for a fractional ambiguity payout: a stale overestimate of another
positive-weight outcome could make an otherwise solvent redemption refuse.
`MAX_OUTCOMES` is 16, so exact whole-vector observation requires no scan and
fits the protocol's bounded-state charter.

## 2. Ordinary transfers and burns

An ordinary Token-2022 transfer changes no mint supply and no Dragon's Clutch
state.  The recipient becomes the bearer.  No originating Position, owner
mapping, or transfer hook is consulted.

An ordinary holder burn is an irrevocable claim forfeiture:

- the next whole-vector synchronization recognizes the lower mint supply;
- recognized claim liability and the derived kernel total fall by the same
  quantity for that outcome;
- Hoard tokens, `HoardAccount.collateral_atoms`, and every Position cash field
  remain unchanged; and
- any decrease in required liability becomes unowned conservative
  overcollateralization inside the retained backing.  V1 has no treasury sweep,
  governance sweep, implicit fee, or owner-recovery claim over it.

Keeping collateral unchanged is conservative and makes a direct burn a safe
donation rather than a market-wide liveness failure.  Decreasing Hoard
accounting at synchronization would be wrong: no claimant was paid and no
Position acquired free cash.

The alternative of restricting burns is rejected for V1.  It would require a
nonstandard Token-2022 policy or additional authority, reduce composability,
and make the protocol's liveness depend on extension semantics that ordinary
Egg holders do not need.

## 3. Pooled-Hoard accounting

The pooled-custody model has four economic classes:

```text
H = actual Hoard Token-2022 amount
L = HoardAccount.collateral_atoms (retained claim backing)
Q = kernel-required collateral for the actual claim vector; Q <= L
C = sum of all Position.cash_atoms (total position cash)
R = sum of all Position.reserved_cash_atoms (a subset of C)
F = C - R (free position cash)
S = unowned surplus from direct Hoard deposits
U = L - Q (conservative overcollateralization, including claim forfeitures)

H = L + C + S              (inductive market-wide relation)
H >= L                     (locally checkable coverage floor)
0 <= Q <= L
0 <= R <= C
S >= 0
U >= 0
```

No one instruction can scan `C` or `R`.  Their equality is established by
account creation and preserved by exact deltas.  The local Hoard check is the
one-sided coverage floor plus exact per-instruction Token-2022 deltas.  A
direct deposit into the public Hoard raises `S`; it creates no Position claim
and cannot brick an exit.

Direct deposits and claim burns have different representations.  A direct
deposit raises `S`.  A direct burn reduces actual claims and therefore may
reduce `Q`, but keeps `L` unchanged; its value is retained in `U` and is not
position cash.

The relevant transitions are:

| transition | Position cash | locked backing `L` | Hoard token `H` |
| --- | ---: | ---: | ---: |
| `Endow(q)` | `+q` | `0` | `+q` exact CPI |
| `Withdraw(q)` | `-q` | `0` | `-q` exact CPI |
| `Split(q)` | `-q` | `+q` | `0` |
| `Merge(q)` | `+q` | `-q` | `0` |
| `RedeemInternal(q)` | `+payout` | `-payout` | `0` |
| `RedeemExternal(q)` | no Position | `-payout` | `-payout` exact CPI |
| direct Egg burn | `0` | `0` | `0` |
| direct Hoard deposit | `0` | `0` | `+q` to surplus |

Hoard principal is never a fee, rent source, bounty, or treasury balance.

## 4. Existing seam cutover

`Split`, `Merge`, `Materialize`, `Dematerialize`, `Resolve`, and
`RedeemInternal` remove the production `ExternalAccount` role.  Every path that
uses the kernel's total-supply vector carries the canonical outcome-mint vector
and runs §1 synchronization before its first economic write.

The exact effects after synchronization are:

- `Split(q)`: debit owner free cash, add `q` to every internal Position and
  internal aggregate, add `q` to every derived total; no token CPI.
- `Merge(q)`: subtract `q` from every internal Position, internal aggregate,
  and derived total, credit owner free cash; no token CPI.
- `Materialize(i,q)`: subtract internal Position and aggregate `q`; mint
  exactly `q` to the authenticated destination; total supply is neutral.
- `Dematerialize(i,q)`: burn exactly `q` from the authenticated source; add
  internal Position and aggregate `q`; total supply is neutral.
- `Resolve`: synchronize the whole vector before freezing the payout.
- `RedeemInternal(i,q)`: subtract internal Position, aggregate, and total;
  lower `L` by the exact payout and credit the same amount to owner free cash;
  no collateral token CPI.

The actor remains the Position owner on all five Position-bound seams.
Materialization and dematerialization name a real holder token account in the
intent and require its Token-2022 owner authority to equal that actor.  The
legacy rule that compared `destination` or `source` to an `ExternalAccount` PDA
is deleted.

## 5. `RedeemExternal`: bearer claim to collateral

### 5.1 Wire intent

The wire action carries:

```text
market
claimant
source_outcome_token_account
destination_collateral_token_account
outcome
quantity
sequence = 0
```

The two account addresses are signed intent bindings, not trusted facts.  The
program compares them to the presented accounts and then independently decodes
their mint and owner authority.

### 5.2 Account plane

Fixed prefix, followed by `market.outcome_count` canonical outcome mints:

| index | account | access |
| ---: | --- | --- |
| 0 | claimant | signer, read-only |
| 1 | Profile | program-owned, read-only |
| 2 | Market | program-owned, read-only |
| 3 | Hoard accounting | program-owned, writable |
| 4 | kernel aggregate | program-owned, writable |
| 5 | SupplyLedger | program-owned, writable |
| 6 | Resolution | program-owned, read-only |
| 7 | immutable Terms | program-owned, read-only |
| 8 | authenticated collateral-policy bytes | read-only evidence |
| 9 | pinned Token-2022 program | executable, read-only |
| 10 | Realm collateral mint | read-only |
| 11 | claimant collateral destination | writable |
| 12 | canonical Hoard authority PDA | read-only |
| 13 | canonical Hoard collateral token account | writable |
| 14 | claimant outcome-token source | writable |
| `15..15+n` | outcome mints in index order | named mint writable; others read-only |

The source outcome mint is one of the canonical suffix accounts, not a duplicate
role.  Account count is exactly `15 + outcome_count`.  The Market carries the
Realm identity needed to derive and bind Profile, Terms, and collateral policy;
a duplicate Realm account would add no authenticated fact to this exit.

No Position, `ExternalAccount`, owner-generation, or owner replay account is
accepted.  A transferred Egg must redeem from a wallet that never had a
Dragon's Clutch Position.

### 5.3 Checks and effects

Before the first write:

1. Decode the request and require `sequence == 0`, nonzero quantity, active
   outcome index, exact account count, distinct keys, declared mutability, and
   claimant signature.
2. Recompute every program PDA and bind Profile, Market, Hoard, Terms,
   Resolution, kernel, SupplyLedger, collateral policy, and token identities.
3. Require the market resolved and the immutable Resolution/Terms evidence to
   select one exact payout vector.
4. Admit the collateral mint, destination, Hoard, all outcome mints, and source
   from current Token-2022 bytes.  Require both claimant token accounts' owner
   authority to equal the signer.  Do not require ATAs.
5. Run whole-vector synchronization (§1), require source amount at least `q`,
   require Hoard coverage, and compute the exact payout with the kernel's
   existing divisibility rule.

Then atomically:

1. burn exactly `q` from the source with the claimant signature;
2. transfer exactly `payout` from Hoard to claimant collateral with the Hoard
   PDA signature (a zero payout performs no value movement but still checks
   unchanged balances);
3. require exact source, mint-supply, Hoard, and destination deltas;
4. persist the synchronized SupplyLedger cache, derived kernel total/collateral,
   and Hoard locked-backing value.

The kernel call uses an ephemeral bearer Position whose sole external balance
is `q` at the named outcome.  It is never persisted and grants no authority;
the admitted source token amount is the precondition.  This reuses the checked
`MarketState::redeem_external` payout and invariant relation without reviving a
per-owner shadow ledger.

### 5.4 Replay semantics

External redemption is a consumptive bearer transition, not a Position command.
V1 therefore has no per-owner program replay PDA for it and requires the common
request envelope's sequence field to be canonical zero.

- Replaying the same signed Solana transaction is rejected by the runtime's
  transaction-signature/recent-blockhash rules (and a durable nonce advances
  atomically when used).
- A third party cannot copy the instruction into a new transaction because the
  claimant signature and both signed account bindings are required.
- A claimant who signs and submits a new transaction authorizes another burn;
  the current source balance is re-read and debited exactly.
- Atomic burn plus supply decrement prevents double redemption of one token
  atom independently of client retry behavior.

If a future product requires an application-level idempotency key across newly
signed transactions, it may add a claimant receipt PDA.  Such a receipt is not
needed for safety and must not be smuggled into the V1 bearer semantics.

## 6. Refusals that must remain stable

- actual outcome mint supply above its last program-observed value;
- a missing, duplicate, out-of-order, wrong-owner, wrong-authority, wrong-mint,
  wrong-decimals, frozen, or extension-bearing token role;
- holder balance below quantity;
- Hoard amount below locked backing or below the computed payout;
- unresolved market, wrong Resolution/Terms binding, invalid outcome, zero
  quantity, arithmetic overflow, or nonintegral payout lot;
- token CPI failure or any non-exact post-CPI delta; and
- any late mismatch, with the SVM rolling every token and program account back.

An observed mint decrease is intentionally not a refusal.  An unsolicited
Hoard increase is intentionally not a refusal.

## 7. Promotion evidence

The cutover is not complete until a real Token-2022 SVM/committed-bank campaign
shows all of the following:

1. Materialize, transfer to a second wallet with no Position, resolve, and
   `RedeemExternal` to that wallet; finish with exact source balance, mint
   supply, payout, Hoard amount, locked backing, and kernel/ledger totals.
2. Burn one materialized Egg directly through Token-2022, then successfully
   run an unrelated claim transition.  Prove the burn is recognized as
   forfeiture, no claimant/cash entry appears, and surplus remains unswept.
3. Transfer outcome tokens across multiple ordinary token accounts and prove
   no per-owner program state is consulted.
4. Donate one collateral atom directly to the Hoard, then complete internal
   and external exits while the atom remains unowned surplus.
5. Mutate every suffix mint key/order/authority and every source/destination
   binding; each mutation refuses before state changes.
6. Force a late failure after a successful burn or transfer CPI and compare all
   watched program/token bytes against the pre-transaction state.
7. Re-run host differentials with the claim boundary stated honestly: the
   offline reference can model arithmetic and state deltas, while actual mint
   and holder truth is evidenced only by the Token-2022 runtime campaign.

Passing those cases is evidence about the pinned local bank and exact ELF.  It
is not an audit, cluster deployment, or proof of the Token-2022 runtime.
