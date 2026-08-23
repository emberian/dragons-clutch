# Atomic structured claims over native Eggs

Status: **RESEARCH MODEL / V1 DECISION NOTE** (2026-08-19). This directory
changes no consensus bytes, production program, account layout, market terms,
deployment artifact, or release claim. It imports no code from another project.

Integration update (2026-08-22): [ADAPTER_PLAN.md](ADAPTER_PLAN.md) audits this
model against the live Terms, Position, reservation, portfolio-settlement,
Split/Merge, redemption, and Token-2022 seams. It supersedes the earlier
identity, custody, descriptor-layout, and CPI recommendations below where they
conflict. In particular:

- the Python model now mirrors the existing live `NativePortfolioClaimV1`,
  wrapped in a separately deployment-bound product id;
- exact rational compiler coefficients now have a minimal integral realization
  with no silent rounding;
- the preferred internal Position holds the common complete-set floor in its
  existing `cash_atoms` and only the residual Egg vector; and
- wrapper composition is flattened to native Eggs and can expose additional
  complete sets without ever persisting a wrapper-under-wrapper edge.

The original exact-Egg vault remains useful as the simpler comparative model.
`CompressedWrapperMachine` exercises the stronger cash-plus-residual design.

## Decision

Dragon's Clutch should keep three distinct things distinct:

1. A **coefficient intent** is one signed, atomically checked order over native
   basis Eggs. It needs no new asset or custody. A fill credits the ordinary
   component Eggs, which may then be moved separately.
2. An **atomic Claim Position** is a persistent program-owned number of lots of
   one canonical coefficient vector. It can preserve product identity, but only
   custom program instructions can transfer or consume it.
3. A **Token-2022 wrapper** is a bearer mint whose every atom is backed by an
   exact basket of native basis Eggs. It adds no market liability and buys real
   external composability, but it creates another mint, custody seam, lifecycle,
   and audit surface.

The V1 recommendation is:

- make bounded coefficient-vector intents native to the coupled batch and keep
  users' settled balances in the existing base Position;
- freeze the canonical structured-claim descriptor and gcd normalization now;
- do **not** make a wrapper necessary to obtain smooth native settlement;
- do not put a generic portfolio-mint factory on the critical path to the first
  trustworthy market; and
- promote an opt-in Token-2022 wrapper only for a selected coefficient claim
  that has a concrete external consumer (spot venue, lending vault, escrow,
  index product, or generic wallet transfer). When promoted, back it with one
  dedicated internal base Position, using its free cash plus residual Egg
  vector, rather than `n` external token vaults.

That is not a recommendation to replace native shaped claims with categorical
baskets. The components here are the market's exact native degree-zero through
degree-three basis Eggs. At resolution, a wrapper with coefficients `a_i` pays

```text
dot(a, w) / D
```

for the native B-spline vector `w`; no categorical cell is selected. A sampled
degree-zero compatibility lowering has a different basis/terms identity and the
model refuses to present it as native.

## What a wrapper atom means

For one frozen native basis with `2 <= n <= 16`, let:

```text
E_i = one atom of native basis Egg i
a_i = nonnegative integer backing coefficient for Egg i
W_a = one atom of the canonical structured-claim wrapper
```

The only mint equation is:

```text
1 W_a  <->  a_0 E_0 + ... + a_(n-1) E_(n-1)
```

Negative coefficients are not assets and cannot be wrapped. They can occur as
signed **trade deltas**, funded by balances and reservations, but a freely
transferable long token cannot carry an unfunded short obligation.

### Canonical scaling

Requested proportional vectors share one wrapper mint. The descriptor divides
all nonzero coefficients by their gcd:

```text
(10, 20, 40) -> primitive coefficients (1, 2, 4), display scale 10
```

Ten atoms of the primitive wrapper exactly reproduce one requested display lot.
The display scale is not part of mint identity. This prevents scalar copies from
fragmenting liquidity while retaining exact integer backing.

The wrapper mint has decimals `0`, matching native Egg mints. Decimals are only
display metadata and cannot make a fractional basket exact. A UI can display a
larger named lot without changing consensus quantities.

V1 wrapper admission refuses:

- the all-zero vector;
- a vector supported on only one Egg, which merely duplicates its native mint;
- a constant positive vector, which is complete-set collateral in disguise;
- more than sixteen coefficients, negative values, non-`u64` values, and any
  scalar multiplication that overflows a vault or supply amount;
- a different Market, basis digest, outcome order, or terms digest; and
- any wrapper, LP receipt, vault share, or arbitrary mint as an underlying.

The live native claim digest binds the Market, immutable Terms digest, degree,
denominator, outcome count, and primitive coefficients. A wrapper-specific
product id additionally binds base/wrapper ProgramData deployments, deployment
slots, Token-2022, and backing-policy version. It deliberately does not bind a
marketing name, analytic compiler label, approximation certificate, or display
scale: two independently reproduced artifacts with the same exact basis,
coefficients, and deployment boundary are the same fungible product.

## Native basis versus compatibility lowering

This wrapper is basis-polymorphic but not semantics-polymorphic. Its descriptor
accepts only `native-open-clamped-bspline-v1`:

```text
authenticated evidence
        -> exact native B-spline evaluator
        -> w, where w_i >= 0 and sum(w_i) = D
        -> wrapper payout dot(a, w) / D
```

If a curve is sampled onto categorical degree-zero cells, the result can be
wrapped only under that categorical Market's own terms. It must retain the
categorical compatibility identity and approximation disclosure. Reusing the
name or coefficients cannot turn that adapter into a native cubic claim.

## Three lifecycle designs

### A. Atomic coefficient intent only

The owner signs `(basis, coefficients, quantity, limit, expiry, nonce)`. The
batch relation reserves and moves the complete bounded vector atomically. It is
the cheapest and most useful default:

- no new mint, descriptor account, vault, or holder account;
- no new supply truth or direct-burn behavior;
- no new Solana CPI path; and
- exact native payout because the output is the native Eggs themselves.

It promises atomic **execution**, not persistent product identity. After the
fill, components are separable bearer or internal balances. A client grouping
them under a name is a view, not an asset.

### B. Program-owned atomic Claim Position

One dedicated backing Position escrows the exact component vector. Per-owner
claim accounts carry wrapper-lot balances, while a canonical descriptor carries
the inductive total. `merge`, `split`, and `transfer` are custom transitions.

This avoids a Token-2022 mint and makes direct burns impossible. It also means:

- every recipient needs a protocol account;
- generic Token-2022 venues, wallets, escrows, and lending protocols cannot use
  the position without an adapter;
- the program-owned total is an inductive supply ledger rather than an
  independently authenticated token mint supply; and
- the protocol must forbid closing a nonzero holder account and prove every
  lot-moving transition updates the same semantic owner.

It is appropriate if atomic identity is needed only inside the native batch.
If the venue already accepts coefficient intents, its incremental product value
is modest.

### C. Trustless Token-2022 wrapper

One canonical mint is derived under the wrapper program from the claim digest.
The actual Token-2022 mint supply is authoritative. A dedicated PDA-owned vault
holds the exact native components and no identity can withdraw them except the
closed wrapper transitions.

Transfers are ordinary Token-2022 transfers. They do not call Dragon's Clutch,
touch the component vault, or serialize on a program Position. This is the one
design that gives the named curve position a conventional bearer identity.

## Baseline custody model and preferred complete-set compression

The most literal wrapper uses one external Token-2022 escrow account for every
nonzero coefficient. It is easy to inspect, but expensive and transaction-heavy:
wrapping a 16-component claim requires sixteen checked transfers plus one mint,
and loads the source account, mint, and vault for every component.

The base protocol already defines its fixed Position as the native claim
representation. The baseline wrapper vault modeled first was therefore:

```text
wrapper claim digest
        -> unique wrapper-vault authority PDA
        -> unique base Position(market, vault authority)
        -> [u64; 16] exact native Egg balances
```

This is not categorical lowering and not an IOU for Eggs. The Position contains
the same native basis claims counted in the base SupplyLedger. It is readable
onchain and controlled only by the wrapper authority PDA.

The audited preferred representation now uses the same Position more fully.
For primitive coefficients `a`, derive `k = min(a)` and `r_i = a_i-k`:

```text
wrapper claim digest
        -> unique wrapper-vault authority PDA
        -> unique base Position(market, vault authority)
        -> k cash atoms + [r_i; 16] internal Eggs per wrapper
```

One cash atom is exactly one merged complete set, so this has the identical
payout under every admitted simplex weight vector. It removes every redundant
complete set from base supply and Hoard collateral while keeping both semantic
facts in the existing Position account. See `ADAPTER_PLAN.md` for the exact
transition, phase, direct-burn, fusion, and deployment rules.

Both internal-Position designs require one small, separately reviewed base
transition before implementation. The audited form also moves free cash:

```text
AtomicPositionAssetTransferV1 {
    source_position,
    destination_position,
    cash_atoms,
    exact [u64; 16] delta,
}
```

It must authenticate the source owner, both canonical Position PDAs, identical
Market and independent generations/replays, zero padding, free cash, checked
quantities, and the source signer; it changes no aggregate Egg supply, Hoard,
or token custody. The wrapper invokes it with the unique vault authority PDA.
Separate donation transitions are needed only to compact direct-burn surplus;
they credit nobody.

If that base seam is not accepted, use external escrow accounts and treat the
53-account/17-CPI worst case as a benchmark gate. Do not silently replace exact
backing with a digest, price oracle, insurance pool, or discretionary keeper.

## Exact transitions in the baseline full-Egg model

The transitions in this section describe the deliberately simpler full-Egg
vault retained in `WrapperMachine`. The promotion design uses the
cash-plus-residual transitions in `ADAPTER_PLAN.md` and
`CompressedWrapperMachine`.

Every transition validates all arithmetic, identities, balances, and account
profiles before the first CPI. Solana transaction rollback is still relied upon
across actual CPIs; the adapter must also check exact post-CPI deltas.

### Merge components / mint wrapper

For quantity `q > 0`:

1. authenticate the canonical descriptor, base program, Market, terms digest,
   wrapper mint, owner, and dedicated vault Position;
2. compute every `q * a_i` in checked `u64` arithmetic;
3. require all source component balances and post-state wrapper/vault amounts;
4. atomically transfer the exact vector into the vault;
5. invoke Token-2022 `MintToChecked` for exactly `q` wrapper atoms; and
6. authenticate the wrapper mint and destination exact deltas.

No collateral enters or leaves the base Hoard and aggregate base Egg supplies do
not change.

### Split wrapper / release components

For quantity `q > 0`:

1. authenticate the holder and exact wrapper mint/account;
2. invoke Token-2022 `BurnChecked` for exactly `q` wrapper atoms;
3. transfer exactly `q * a_i` of every component from the dedicated vault to the
   owner's base Position; and
4. authenticate exact post-state supply, balance, and vector deltas.

The order is not an economic trust choice because failure of either leg rolls
the whole Solana transaction back. The program nevertheless computes the entire
post-state before invoking either external program.

### Transfer

A transfer is an ordinary Token-2022 transfer of `W_a`. It changes holder
balances but neither actual mint supply nor vault coverage. No transfer hook is
needed: the token atom already is the atomic claim.

### Direct holder burn

Token-2022 permits an owner to burn their wrapper without invoking the wrapper
program. Therefore the program must not maintain a supply shadow that assumes it
saw every burn.

Let `S` be authenticated current wrapper-mint supply and `B_i` the authenticated
vault balance. The invariant is:

```text
B_i >= S * a_i  for every i
```

A direct burn decreases `S` without decreasing `B`, so it can only improve
coverage. The surplus is a donation. It never becomes a caller bounty, fee,
treasury asset, or operator withdrawal.

A permissionless `CompactDonation` may destroy exactly
`B_i - S*a_i` surplus native Eggs through the base donation transition. This
reduces base-market liability and allows eventual retirement. Leaving the
surplus locked is also safe. Transferring it to a person is not.

### Terminal redemption

The always-available conservative path is:

```text
split W_a into exact native Eggs -> use the base Market's frozen redemption rule
```

An optimized direct wrapper redemption is economically valid but needs a new
base-program aggregate redemption CPI. For quantity `q`, it atomically burns the
wrapper, consumes `q*a_i` vault Eggs, and pays exactly:

```text
q * dot(a, w) / D
```

It must refuse unless the amount is integral or persist an explicit remainder
credit. Silent floor rounding and dust-to-treasury are forbidden.

The conservative least quantity that is integral for every integer simplex
weight vector is:

```text
L = lcm_i D / gcd(D, |a_i - a_0|)
```

After one resolution vector is frozen, the smaller exact lot is:

```text
L_w = D / gcd(D, dot(a, w))
```

The executable model checks the universal formula and its minimality over small
integer simplexes. V1 should not advertise direct wrapper-to-collateral
redemption until the aggregate base transition, exact-lot UX, and SBF rollback
path exist. Unwrapping must remain available; an inexact holder is never seized.

### Retirement

Retirement requires authenticated wrapper mint supply zero and vault balances
zero after donation compaction. An extension-free Token-2022 mint has no close
authority, so its small rent deposit remains as a canonical tombstone. The
descriptor should likewise remain immutable or close only into a permanent
minimal tombstone; closing and recreating the same economic identity under new
semantics is forbidden.

## Solvency

Let `T_i` be total base supply of native Egg `i`, including all wrapper vaults.
Let native settlement weights obey `w_i >= 0` and `sum(w_i) = D`. The base
Market's liability is:

```text
L_base(w) = sum_i T_i*w_i/D
```

Wrapping transfers existing Eggs into a vault. It does not change any `T_i`.
Minting `W_a` therefore adds no base claim and no Hoard liability. Unwrapping and
ordinary wrapper transfers also preserve `T_i`.

For wrapper supply `S`, coverage gives every holder an exact pro-rata claim on
already-counted components:

```text
B_i >= S*a_i
```

Direct wrapper burns reduce `S`; component donations increase `B`; both improve
coverage. Donation compaction burns components and lowers `T_i`, which can only
lower `L_base`. Exact aggregate terminal redemption reduces `T_i` by `q*a_i`
and the Hoard by the identical `q*dot(a,w)/D`, preserving resolved solvency.

The wrapper must never be counted as a second liability in the base
SupplyLedger. It is a custody receipt over claims that are already counted.

## Nesting refusal

Wrapper-of-wrapper composition is deliberately absent. The descriptor does not
store an arbitrary mint list. It derives exactly one canonical native Egg mint
per outcome from `(base program, Market, basis terms, index)` in canonical order.
Consequently a wrapper mint cannot satisfy the underlying derivation.

Nesting would otherwise create recursive solvency walks, ambiguous direct-burn
surplus, multiple retirement dependencies, CPI-depth pressure, and circular
backing. Any economically useful composition of structured claims can be
flattened offline into one nonnegative coefficient vector over the native basis,
then gcd-normalized and checked once. Signed/negative composition belongs in a
funded order relation, not a bearer wrapper.

## Token-2022 profile

The candidate wrapper mint is intentionally boring:

| property | V1 wrapper policy |
| --- | --- |
| token program | pinned Token-2022 deployment |
| decimals | `0` |
| mint authority | canonical wrapper-authority PDA |
| freeze authority | none |
| mint extensions | none; exact 82-byte base mint |
| holder account extensions touched by wrapper | none or `ImmutableOwner` only |
| external vault account, if used | `ImmutableOwner`, PDA owner, no delegate, no close authority |

In particular it refuses TransferFee, ConfidentialTransfer, DefaultAccountState,
NonTransferable, InterestBearing, PermanentDelegate, TransferHook,
MintCloseAuthority, metadata/group pointers, ScaledUiAmount, Pausable, and
PermissionedBurn. Metadata is a content-addressed client artifact, not mutable
mint authority.

No TransferHook is needed to preserve atomicity. Adding one would make every
otherwise ordinary transfer load another program and account set, and would
reduce precisely the composability the wrapper is meant to buy.

## Superseded candidate descriptor layout

The 272-byte layout below predates deployment-slot binding and the decision to
derive, rather than persist, redundant claim/product ids. Do not implement it.
`ADAPTER_PLAN.md` specifies the current 384-byte candidate.

The resource model uses this 272-byte proposal:

| field | bytes |
| --- | ---: |
| tag, version, flags | 4 |
| base program | 32 |
| Market | 32 |
| native terms digest | 32 |
| claim digest | 32 |
| `[u64; 16]` primitive coefficients | 128 |
| outcome count, state, descriptor bump, mint bump, vault bump | 5 |
| reserved zero bytes | 7 |
| total | 272 |

The descriptor has no wrapper-supply field. Token-2022 owns that fact. It has no
analytic expression, arbitrary bytecode, mutable metadata pointer, price,
resolver, collateral balance, or operator authority.

## Account, rent, and CPI estimates

These are arithmetic design estimates, not compiled-SBF measurements. Rent uses
Solana's default `3,480` lamports per byte-year, exemption threshold `2.0`, and
128-byte storage overhead. One 170-byte `ImmutableOwner` Token-2022 account costs
`2,074,080` lamports; an 82-byte mint costs `1,461,600` lamports.

Assuming all outcomes have nonzero coefficients:

| n | external-vault infrastructure | internal-Position infrastructure | position-only infrastructure | external wrap accounts / CPIs |
| ---: | ---: | ---: | ---: | ---: |
| 2 | 0.008393760 SOL | 0.008143200 SOL | 0.006681600 SOL | 11 / 3 |
| 4 | 0.012541920 SOL | 0.008143200 SOL | 0.006681600 SOL | 17 / 5 |
| 8 | 0.020838240 SOL | 0.008143200 SOL | 0.006681600 SOL | 29 / 9 |
| 16 | 0.037430880 SOL | 0.008143200 SOL | 0.006681600 SOL | 53 / 17 |

The external-vault infrastructure is descriptor + wrapper mint + `n` escrow
token accounts. A wrap loads five common accounts plus source account, native
mint, and vault per component, and invokes `n` checked transfers plus one wrapper
mint/burn.

The historical internal-Position estimate is descriptor + wrapper mint + one current
220-byte base Position + one current 84-byte Replay account. It estimates twelve
accounts and two outer CPIs per merge/split: one base vector transfer and one
Token-2022 mint/burn. It is independent of `n` up to the fixed sixteen-outcome
layout. A holder ordinarily needs one 170-byte wrapper token account either way.

The position-only estimate omits the wrapper mint, uses the same dedicated base
Position/Replay, and assigns a proposed 112-byte atomic claim account to each
holder. It is not generic Token-2022 composability.

With the audited 384-byte descriptor, the internal Position design is estimated
at `0.008922720 SOL` rather than `0.008143200 SOL`. The extra bytes bind the
checkable deployment boundary; no program rent estimate has been measured yet.

At `n=16`, 53 loaded accounts may fit a runtime ceiling but the transaction
message and compute still need a real SBF benchmark, likely with address lookup
tables or staged funding. Staging creates an orphan/recovery state machine and
must not mint until all component vaults are complete. The internal Position is
the materially cleaner path.

## When the wrapper has product value

A wrapper earns its complexity when at least one actual consumer requires a
single bearer asset:

- a Token-2022 spot venue should trade the shaped claim atomically;
- a lending or collateral vault should custody one mint rather than reconstruct
  sixteen balances;
- users should transfer a named curve position through ordinary wallet flows;
- an escrow, multisig, vesting contract, or index should hold the complete shape;
- holder account/rent compression matters for a broad-support claim; or
- aggregate exact redemption produces a materially smaller lot than independent
  component redemption.

It does not earn its complexity for a one-off batch fill, a UI grouping, a
single Egg, a complete set, a claim with no external liquidity destination, or
an analytic label that lacks a basis-bound coefficient artifact. Permissionless
creation of every possible coefficient vector would fragment liquidity across
an enormous catalog. Canonical gcd normalization removes scalar duplicates but
does not solve product discovery; creation rent should be prepaid by the creator
and no protocol principal should subsidize it.

## Promotion gates

Before a wrapper can leave research, require all of the following:

1. exact canonical codec/digest vectors and PDA derivations;
2. immutable binding to the base program deployment, Market, and native terms;
3. an accepted internal vector-transfer transition or measured external-vault
   implementation;
4. authenticated actual wrapper mint supply and base Position/vault balances;
5. exact pre/post CPI delta checks and adversarial rollback tests against the
   real Token-2022 program;
6. direct wrapper-burn donation and permissionless surplus-compaction tests;
7. account-substitution, foreign-Market, nested-wrapper, delegate, extension,
   overflow, and close-state refusals;
8. `n = 2, 4, 8, 16` SBF account/message/CU measurements;
9. a lifecycle ordering that keeps split available through base redemption and
   retires only at zero actual supply and zero vault balance;
10. an aggregate redemption theorem and remainder policy before offering direct
    wrapper-to-collateral redemption; and
11. a named external integration that justifies the extra surface.

Until those pass, coefficient intents remain fully native shaped trading; the
absence of a wrapper is not permission to call the result categorical.

## Executable evidence

Run:

```sh
python3 -m unittest discover -s research/structured-claim-wrapper -p 'test_*.py' -v
python3 research/structured-claim-wrapper/run_lab.py
```

The tests cover:

- descriptor normalization and basis-identity separation;
- exact rational-to-integral live claim realization and deployment-bound
  wrapper identity in the Rust compiler bridge;
- compatibility-lowering, nesting, foreign-basis, and redundant-wrapper
  refusals;
- exact merge, split, transfer, direct burn, donation, and retirement behavior;
- complete-set cash compression, canonical-backing mint/unwind, post-resolution
  phase-independent release, surplus compaction, and payout equality with the
  full-Egg vault;
- actual supply versus holder balances and per-component coverage;
- unbacked mint, vault drain, overflow, and validate-before-mutate attacks;
- direct aggregate native B-spline payout rather than categorical selection;
- exact resolved lots and exhaustive universal-lot minimality over small
  integer simplexes;
- 5,000 deterministic mixed adversarial transitions in each backing model; and
- the bounded rent/account/CPI arithmetic above.

Passing tests are evidence about this model. They are not a proof about an SBF
adapter, Token-2022 runtime, base-program CPI, B-spline evaluator, or deployed
program.
