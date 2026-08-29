# Exact economic lifecycle ledger

This directory is the offline, exact-integer oracle for PRIVATE and the
corrected devnet-only Activity-v3 campaign. It predicts economic poststate from
source-owned inputs; it never reads RPC, keys, browser state, or a deployed
program.

The two canonical contracts are:

- `fixtures/private-canonical.json`: current PRIVATE founding through aggregate
  retirement.
- `fixtures/activity-v3-canonical.json`: corrected ten-wallet devnet-only
  Activity-v3 authority and the four-participant flagship economic ensemble.

The deterministic model-based companion is:

- `fixtures/multiwallet-20-seeds.json`: the exact ordered seed/profile contract.
- `fixtures/multiwallet-20-seeds.expected.json`: canonical expected snapshots
  for all twenty seeds, bound to both source-fixture byte digests.

`multiwallet.py` owns only control-flow expectations around the economic
ledger. `ledger.py` remains the single semantic owner for collateral, Claims,
Hoard principal, Direct fee arithmetic, redemption, and conservation.

The old `tools/devnet-scenarios/fixtures/flagship.json` remains intentionally
untouched and scenario-only. It is not admissible Activity-v3 authority: its
adapter-required Direct/redeem/retire operations have
`mutationExpected=false`, it has no five-wallet zero-prefunded campaign signer
partition, and its 150,000,000-lamport payer cannot cover four 50,000,000
post-init transfers plus fees. `check-activity-v3-scenario` refuses it instead
of weakening `activity.py`.

## Exact model

All JSON quantities are canonical unsigned decimal text and all asset domains
remain separate.

For each Direct fill `q`, price `p`, price scale `S`, fee basis points `b`, and
denominator `D`:

```text
q * p = gross * S                     (remainder must be zero)
fee_per_side = floor(gross * b / D)   (the one named rounding boundary)
seller_net = gross - fee_per_side
buyer_debit = gross + fee_per_side
fee_credit = 2 * fee_per_side
seller_net + fee_credit = buyer_debit
```

Products are bounded as u128 and every persisted amount is bounded as u64.
Both canonical fixtures bind `b=50`, `D=10000`.

The model checks after every stage:

```text
sum(all Token-2022 collateral accounts) = immutable collateral Mint supply
Claims aggregate[i] = sum(all Position[i])
before resolution: Claims aggregate[i] = Hoard principal, for every i
after resolution:  Claims aggregate[winner] = remaining Hoard principal
```

Hoard principal is always classified as collateral principal. It is never a
fee, bounty, rent, reserve, treasury source, or revenue. Losing claims still
execute positive claim burns with zero collateral payout. Retirement refuses
until every winning and losing Position coordinate, every Claims aggregate
coordinate, and the Hoard principal are exactly zero.

## Canonical PRIVATE vector

The current source vector is:

```text
Token-2022 supply                 1,100,000,000 atoms
founding collateral              1,000,000,000
participant fixture supply         100,000,000
founding complete set Q            500,000,000
Direct fill                        100,000,000
Direct execution price / scale       500,000 / 1,000,000
Direct gross                        50,000,000
fee per side                           250,000
seller net                          49,750,000
buyer debit                         50,250,000
fee recipient credit                   500,000
```

The pinned Pyth fixture has raw price `100000000`, exponent `-8`; against the
demo cuts `12000/100` and `18000/100`, it selects outcome 0. The frozen Direct
payout schedule is therefore founder quantities
`[400000000,500000000,500000000,500000000]` plus participant outcome-0
quantity `100000000`. All five positive claims burn. Winner payouts total
500,000,000, leaving Claims and Hoard at zero.

Final collateral accounts are exactly:

```text
founding collateral wallet       500,000,000
founder Direct recipient          449,750,000
participant recipient             149,750,000
Direct fee recipient                  500,000
participant fixture source                  0
Hoard principal                             0
```

## Corrected Activity-v3 authority

The corrected devnet-only authority has exactly ten wallets: deployer; five
distinct campaign signer roles initially at zero; and ash, birch, cobalt,
dahlia. The payer receives 360,000,000 lamports. Each participant receives one
50,000,000-lamport post-init transfer after founding.

```text
post-init transfer principal      200,000,000
maxPostInitTransferLamports       200,000,000
maxPostInitFeeLamports             10,000,000
maxFeeLamports                     10,000,000  (distinct full-campaign ceiling)
maxSpendLamports                  210,000,000
guaranteed payer residual         150,000,000
```

Actual post-init fees and actual full-campaign activity fees remain distinct
reconciled quantities even though both ceilings are 10,000,000. This is an
authorized devnet spend envelope, not mainnet evidence.

The flagship atom vector mints four complete sets of
`1000 + 700 + 400 + 250 = 2350` atoms, executes the four source-authored Direct
fills, freezes fourteen nonzero payout rows, selects outcome 2, and burns all
fourteen. Protocol fees total 4 atoms; winning payouts return all 2,350 Hoard
atoms. Final participant collateral is ash 50,010, birch 50,249, cobalt 49,548,
dahlia 50,189, plus the 4-atom fee account.

## Lamports, Rent, refunds, and wallet envelopes

Rent amounts are deliberately runtime parameters. The source obtains them from
the authenticated Rent sysvar (`minimum_balance`) for the exact account widths;
hard-coding a past cluster quote here would create a second truth. Feed the
observed exact funding, fee, rent-lock, and rent-refund events to
`check-lamports`.

For each wallet, the exact required starting envelope in transaction order is:

```text
peak_prefix(
    outgoing transfer principal
  + refundable rent locks
  + network transaction fees
  - incoming transfer principal
  - exact rent refunds
)
```

Only network fees are destroyed. Transfers and rent locks preserve lamports;
rent closures credit the named beneficiary exactly. PRIVATE requires one exact
100,000,000,000-lamport genesis transfer. Its source wallet spends that
principal plus the funding transaction fee.

PRIVATE aggregate retirement binds exactly five refund classes:

```text
aggregateRefund = market + rentCredit + claimsRefund + custodyReplay + hoardVault
terminalRefundWallet = refundWalletBefore + aggregateRefund
                     - four aggregate transaction fees
                       (only when refund wallet is also fee payer)
```

The trace checker rejects altered refund amounts, missing or extra refund
classes, repeated rent accounts, changed beneficiaries through the resulting
wallet vector, and any failure of
`sum(wallet deltas) + live rent + network fees = 0`.

## Runner use

Derive all predicted snapshots:

```sh
python3 tools/economic-lifecycle-ledger/ledger.py derive \
  tools/economic-lifecycle-ledger/fixtures/private-canonical.json
```

Before advancing an expensive stage, capture the model-shaped snapshot with
schema `dclutch-exact-economic-lifecycle-observed-snapshot-v1`, the fixture
SHA-256 printed by `derive`, the stage id, and the exact `snapshot` object. Then:

```sh
python3 tools/economic-lifecycle-ledger/ledger.py check FIXTURE.json OBSERVED.json
python3 tools/economic-lifecycle-ledger/ledger.py check-lamports FIXTURE.json TRACE.json
```

This makes the runner compare source predictions before/after founding,
participant admission, Direct, resolution, every grouped payout frontier, and
aggregate retirement instead of waiting for a validator failure to discover an
economic model mismatch.

Offline verification:

```sh
python3 -m unittest tools/economic-lifecycle-ledger/test_ledger.py
```

The adversarial suite covers indivisible gross quotes, u128 overflow, per-side
fee rounding, altered/missing payout quantity, omitted zero-payout loser burns,
premature retirement, fee-to-Hoard substitution, collateral supply mismatch,
fixture-digest substitution, Rent refund mismatch/class omission, authorization
cap changes, nonmutating Activity-v3 gaps, and the old scenario-only flagship.

## Twenty-seed model-based oracle

The multiwallet contract derives the same PRIVATE named-seed digest domain for
`seed-01` through `seed-20`. Each seed carries an ordered transition log with
the full economic/control post-snapshot, pre/post SHA-256, and explicit
conservation vector. It covers crossed per-maker Direct nonces, stale/future
nonces, an exact duplicate paired intent, simultaneous actions sharing a
seller or buyer nonce, signed collateral-account switching and foreign-account
refusal, every exact cut boundary, provider failure, the gross-199/gross-200
50-bps fee floor, stale replay, payout resume, zero-payout losing burns, and
retirement-before-zero refusal.

“Paired intent” is the generator's stable name for the two signed inline
Direct actions settled together; it does not introduce a second onchain ticket
type. Simultaneous candidates are sorted by their paired-intent SHA-256. The
first valid candidate consumes both makers' exact next nonces atomically; a
candidate sharing either consumed nonce then refuses without changing state.
Signed collateral accounts are checked against their owner on every action and
are deliberately not sticky across the campaign.

Winner selection uses integer rational cross-products. Successful prices at
`11999/100`, `12000/100`, `17999/100`, `18000/100`, and `18001/100` select
outcomes 0, 1, 1, 2, and 2 respectively; provider failure selects outcome 3.
No float enters the oracle. Payout progress always burns one complete positive
row from the frozen schedule. Losing rows transfer zero collateral but remain
mandatory burns. The resume seed stops only between rows and binds the same
frozen-schedule digest before continuing.

Regenerate or print the canonical ensemble entirely offline:

```sh
python3 tools/economic-lifecycle-ledger/multiwallet.py derive \
  tools/economic-lifecycle-ledger/fixtures/multiwallet-20-seeds.json

python3 tools/economic-lifecycle-ledger/multiwallet.py emit \
  tools/economic-lifecycle-ledger/fixtures/multiwallet-20-seeds.json \
  tools/economic-lifecycle-ledger/fixtures/multiwallet-20-seeds.expected.json
```

PRIVATE and Activity-v3 can compare a captured poststate without RPC work in
the oracle. Provide schema `dclutch-model-based-multiwallet-observed-v1`, the
derived `contractSha256`, `seedName`, transition `ordinal`, and exact
model-shaped `snapshot`:

```sh
python3 tools/economic-lifecycle-ledger/multiwallet.py check \
  tools/economic-lifecycle-ledger/fixtures/multiwallet-20-seeds.json \
  OBSERVED.json
```

The check refuses another contract, missing/ambiguous seed, absent ordinal, or
any economic/control difference. The generator never opens RPC, reads a key,
builds, signs, deploys, or submits a transaction.

```sh
python3 -m unittest \
  tools/economic-lifecycle-ledger/test_ledger.py \
  tools/economic-lifecycle-ledger/test_multiwallet.py
```
