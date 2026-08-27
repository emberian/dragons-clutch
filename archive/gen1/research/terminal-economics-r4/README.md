# Terminal economics R4

Status: **MODEL-ONLY / HOST-TESTED / NOT A PROGRAM ABI**.

This isolated, dependency-free `no_std`/`no_alloc` crate is a bounded witness
for a creation-only terminal profile. It changes no SBF program, account,
Token-2022 mint, live authority, market term, release artifact, or legacy
classification. The manual Policy, CreditRoot, claimant Credit, mint binding,
rent, and tombstone codecs are a proposed serialization cut for review, not
deployed bytes.

The model answers one narrow question: what exact state must survive when new
markets permit arbitrary-quantity external bearer claims and native collateral
cannot represent every fractional payout?

## Result

Arbitrary raw bearer quantities and tombstone-only closure are incompatible in
general. R4 therefore preserves a permanent, segregated `CreditVault`, a
`CreditRoot`-like total in the model, and any nonzero claimant credit accounts.
The ordinary market graph can close after all claims and closeable mints reach
zero, but fractional claimant rights do not get swept, rounded away, or paid to
an arbitrary owner.

An alternative future profile can recover tombstone-only closure by requiring
an exact-lot bearer encoding at issuance/bridge. That is a different immutable
creation policy; it is not an in-place migration of raw-unit bearer mints.

## Supply planes

For outcome `i`:

```text
I_i = internal Position claim quantity
E_i = registered/accounted external bearer quantity
A_i = authoritative Token-2022 mint supply
T_i = I_i + E_i
```

`T_i` is not mint supply. The bounded model requires `E_i = A_i` after every
authenticated transition. `materialize` moves `I_i -> E_i` and increases `A_i`;
external redemption decreases `E_i` and `A_i`; observed third-party burn
reconciliation supplies the complete registered bearer vector for that outcome
and requires the sum of bearer deltas to equal the authenticated mint delta.
The burned quantity is recorded separately. A supplied owner identity is not
proof that a burn occurred.

The fixed bearer array is only a host witness. A live adapter cannot discover
all Token-2022 accounts by scanning instruction inputs. It needs a persistent
aggregate SupplyLedger and must bind every transition to canonical mint and
token-account post-state.

## Exact rights conservation

Resolution freezes nonnegative weights `w_i` with denominator `D` and
`sum(w_i)=D`. Let:

```text
Q = sum_i (I_i + E_i) * w_i       remaining claim numerator
K = sum_owner r_owner             outstanding credit numerator
B = sum_i burned_i * w_i          direct-burn forfeiture numerator
X = owner-authorized credit forfeiture numerator
P = collateral atoms already paid from Hoard or CreditVault
N = complete-set collateral issued into Hoard
```

The model maintains:

```text
D * N = Q + K + D * P + B + X
```

For an owner redeeming quantity `q` of outcome `i`:

```text
n  = r_owner + q * w_i
p  = floor(n / D)
r' = n mod D
```

It atomically pays `p`, replaces `r_owner` with `r'`, and burns/decrements the
claim. One canonical credit account per owner prevents identity splitting
inside a credit domain.

## Why a credit vault remains

At sealing, all internal/external claims are zero, all Positions are closed,
and all creation-time closeable outcome mints are closed. The model transfers:

```text
V = ceil(K / D)
```

collateral atoms from Hoard to CreditVault. Define:

```text
U = D * V - K,  where 0 <= U < D
D * V = K + U
```

`U` is segregated rounding slack, not protocol revenue. Unsolicited tokens sent
to the vault are tracked in a distinct donation balance and never increase
credit backing or `U`. Owners may voluntarily
transfer credit into another owner-accepted credit account. If a transfer makes
the destination numerator cross `D`, the vault pays the resulting whole atom.
Only an authenticated owner forfeiture can reduce that owner's credit; it sends
only newly excess whole vault atoms to the immutable neutral sink.

Counterexample: with `D=2`, Alice and Bob each own numerator `1`. Aggregate
`K=2`, but neither individually owns a whole atom. Paying either person one atom
takes half of the other's right. There is no exact, deterministic,
no-confiscation terminal rule over indivisible collateral unless owners consent
to a merge, collateral has finer units, a separately funded rounding rule is
chosen, or issuance used exact lots.

## Hoard collateral and donations

Hoard maintains the local asset equation:

```text
issuance_in + donations_in
  = balance + redemption_out + credit_vault_out + surplus_sink_out
```

Unsolicited collateral donations create no claim, refund, keeper reward, fee,
or named-donor right. After claim liabilities are moved or paid, surplus goes
only to the immutable neutral sink. Direct-burn slack remains distinct from
donations in rights accounting even when both ultimately contribute to terminal
surplus.

At terminal the model also checks:

```text
D * (Hoard donations + CreditVault donations) + B + X
  = D * (Hoard surplus sunk + CreditVault excess/donations sunk
         + live CreditVault donations) + U
```

## Rent, refunds, and replay

Every modeled account has an exact creation-time `RentRecord`:

```text
principal + donations = live_balance + principal_refund + donation_sink
```

Principal has one immutable refund owner. Excess lamports have no right to that
refund and go to the neutral sink on close. Keeper funds use a separate prepaid
ledger:

```text
keeper_deposit + donations
  = live_balance + rewards + refund + donation_sink
```

The creation-time permanent Replay account is independently funded and reserves
the market/generation before the first active model can exist; a second creation
against that registry refuses. It cannot depend on already-closed Market bytes.
Its tombstone freezes the market and
generation, terminal receipt, final market nonce, CreditVault identity, exact
rent role bitsets/totals at market-graph close, and keeper disposition. Later
credit-account close activity and later donations to permanent accounts are
monotone relative to that frozen snapshot.

## Account classes

- `RefundableTransient`: new R4 market graph, Positions, and creation-time
  closeable mints.
- `PermanentInfra`: CreditVault and declared legacy no-close mints.
- `PermanentTombstone`: independently prepaid Replay receipt.
- `ExternalOwnerState`: claimant credit accounts, until their owner reaches
  zero and closes them.
- `UnclassifiedStop`: legacy or malformed state whose identity, rent source, or
  authority cannot be proven.

Legacy mints without a creation-time MintCloseAuthority are permanent
infrastructure. R4 never adds an extension in place and never calls them
closeable.

## Run

```sh
cargo test --manifest-path research/terminal-economics-r4/Cargo.toml
cargo test --release --manifest-path research/terminal-economics-r4/Cargo.toml
cargo clippy --manifest-path research/terminal-economics-r4/Cargo.toml \
  --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc \
  --manifest-path research/terminal-economics-r4/Cargo.toml --no-deps
```

See [`MODEL_BOUNDARY.md`](MODEL_BOUNDARY.md) for the live adapter STOPs and
[`PROOF_ARGUMENTS.md`](PROOF_ARGUMENTS.md) for the bounded arithmetic argument.
