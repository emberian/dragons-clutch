# Signed committed SBF walk

Status: **implementation in progress; no green Clutch walk is claimed by this
document until the evidence block below is filled by the gate itself.**

The existing SBF differential harness proves real SBF execution against exact
post-state bytes, but it deliberately calls `simulateTransaction` with
`sigVerify: false`.  Its conceptual lifecycle also uses a separate market
identity for each prestate because simulations do not commit.  This lane asks
a different and narrower question:

> Can fresh local test keypairs sign the actual transaction messages, can a
> loopback `solana-test-validator` commit them in order against one market
> identity, and do confirmed account reloads match the declared offline state
> after every step?

The dedicated runner lives in
`programs/clutch-sbf/committed-harness`.  It refuses every non-loopback URL,
requires payer and actor keypairs whose public keys exactly match the first two
message signer keys, fetches a fresh blockhash, signs the serialized message,
submits with `sendTransaction`, waits for confirmation, and reloads each
declared account.  Expected refusal transactions are sent past preflight so the
local bank records the failure; all watched state must remain byte-identical.

## What this still cannot mean

This is not presently an end-to-end permissionless market lifecycle.

`CreateMarket` expects these eight zeroed, program-owned PDAs to exist before
the instruction begins:

1. `MarketAccount`;
2. `HoardAccount`;
3. the founding `PositionAccount`;
4. `KernelAccount`;
5. the founding `ExternalAccount`;
6. the founding `ReplayAccount`;
7. `SupplyLedgerAccount`; and
8. `ResolutionAccount`.

An ordinary wallet cannot sign for those PDAs or assign arbitrary accounts to
the Clutch program.  Until the program itself creates them through System
program CPI, the local validator must inject them at genesis.  The plan records
`genesis_assisted: true`, enumerates the accounts, and the runner prints `NOT
END TO END` before it submits a transaction.

The repair must be an atomic initializer, most naturally inside
`CreateMarket`, which:

- derives and validates all eight targets before value moves;
- requires genuinely absent targets, rather than trusting pre-owned zero
  storage;
- funds rent from the authenticated creator;
- uses the frozen PDA seeds with `invoke_signed` to allocate and assign each
  exact account length;
- encodes the initial state only after every account exists; and
- lets transaction rollback erase the entire partial creation on any failure.

The same mechanism is not sufficient for a multi-user venue.  A separate
permissionless actor-plane initializer must create `PositionAccount`,
`ExternalAccount`, and `ReplayAccount` for each later `(market, owner,
generation)` identity.

Two later product exits are also absent:

- there is no `Withdraw` intent.  A backed `Endow` can move Token-2022
  collateral into pooled custody and credit position cash, and current seam
  operations can reclassify cash and locked backing, but the owner cannot move
  free position cash back out of the Hoard;
- there is no external-claim redemption instruction.  Materialized outcome
  tokens can be dematerialized before resolution, but any left outstanding at
  resolution cannot presently claim its payout directly.

The committed walk must therefore stop at the strongest honest boundary.  It
may prove a signed, sequential, confirmed state trajectory from a
genesis-assisted prestate.  It must call the terminal free cash **stranded**,
not withdrawn, and any materialized terminal claim **outstanding**, not
settled.  `SettlePage` is separately unimplemented, so no committed trading
venue lifecycle is implied.

## Endow backing

The old `Endow` was an internal-ledger-only credit and could not support an
economic committed walk.  The pooled-custody repair changes its value boundary
to an authenticated Token-2022 `TransferChecked` from the actor to the Hoard,
with exact pre/post debit and credit checks, before the position cash and
replay bytes are committed.  The committed gate must not be marked green until
it observes all four facts in one confirmed transaction:

- actor token balance falls by the endowed quantity;
- Hoard token balance rises by the same quantity;
- position cash rises by the same quantity; and
- replay sequence advances by exactly one.

Split, Merge, and internal Redemption then reclassify assets already in pooled
custody and must not perform a second Token-2022 debit or payout.  Their
confirmed reloads must show unchanged custody-token balances alongside the
declared cash/locked-liability changes.

## Evidence

Pending.  The final block will name the repository commit, SBF ELF digest,
Agave versions, exact signed step list, transaction signatures from the
ephemeral local ledger, confirmation level, post-state comparison count, and a
negative run whose deliberately corrupted expectation turns the gate red.
Local signatures and ledger entries are disposable test evidence, not cluster
transactions or deployment receipts.
