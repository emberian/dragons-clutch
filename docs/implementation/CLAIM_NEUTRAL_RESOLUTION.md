# Claim-neutral resolution and the outcome-mint vector

Status: **ISOLATED MODEL GREEN; LIVE CHANGE NOT AUTHORIZED OR IMPLEMENTED**,
2026-08-19.

This note audits one optimization: whether `Resolve` may record the immutable
market payout without receiving all canonical Token-2022 outcome mints. The
answer is:

> **Yes for economic safety and eventual payout equivalence on reachable
> states, if the exact invariants below remain inductive and every
> payout-moving consumer synchronizes current mint truth first. No as a
> byte-for-byte/refusal-for-refusal replacement for the present handler.**

The irreducible difference is detection time. A mint-free instruction cannot
detect an unaccounted increase in an omitted mint. The current handler detects
that fault during first resolution; a claim-neutral handler would record the
same payout and the first later claim consumer would refuse before moving
value. If immediate detection during `Resolve` is a required property, omitting
the vector is a **STOP**.

No SBF, layout, dispatch, shared crate, or live account ABI was changed by this
lane. The dependency-free executable model is
`research/claim-neutral-resolution`.

## 1. Exact current truth

### 1.1 Mint authority and external truth

`programs/clutch-sbf/program/src/instructions/market_init.rs::create_token_plane`
creates every canonical outcome mint at its outcome-index PDA. Each is an
extension-free, zero-decimal Token-2022 mint with the Market PDA as its mint
authority and no freeze authority.
`programs/clutch-sbf/program/src/token.rs::MintPolicy::outcome` re-admits that
shape whenever a live claim transition observes a mint. Because only this
program can sign for the Market PDA, ordinary holders may transfer or burn but
cannot mint. This is an authorization/reachability fact, not a property that a
transaction omitting the mint can re-check.

For active outcome `i`, current production state has three related quantities:

```text
I_i = SupplyLedger.internal_supply[i]
C_i = SupplyLedger.external_supply[i]       (last observed mint supply)
A_i = current canonical Token-2022 mint supply
T_i = KernelAccount.total_supply[i]
```

`programs/clutch-sbf/program/src/claim_truth.rs` owns the boundary:

```text
T_i = I_i + C_i
A_i <= C_i
```

The first equality is exact program-state closure. The inequality is the
conservative-cache reachability invariant. A holder burn makes it strict. The
next full synchronization requires the equality first, refuses `A_i > C_i`,
then writes `C_i := A_i` and `T_i := I_i + A_i` with checked arithmetic.
Actual mint bytes remain bearer-supply authority; the cache is never holder
balance authority.

### 1.2 What current Resolve does

`programs/clutch-sbf/program/src/instructions/observe_resolve.rs::resolve_global`
currently accepts:

- legacy-buffer finite-preset and native-point Resolve: fixed prefix `11`,
  then `n` canonical outcome mints;
- archive-direct V2 degree-zero categorical and native-occupation Resolve:
  fixed prefix `10`, then `n` canonical outcome mints.

All mints are read-only. Before the semantic transition, the handler derives
and admits every canonical mint and records the full supply vector. On the
first successful resolution it:

1. validates the cached SupplyLedger/kernel closure;
2. derives and records the immutable payout through the selected v2, v3, or v4
   path;
3. synchronizes any holder burns into SupplyLedger and kernel totals;
4. re-observes every mint and requires an exact zero delta; and
5. relies on transaction atomicity to roll back the earlier lifecycle writes
   if a late synchronization or delta check refuses.

An exact Resolve replay does not reconcile a later holder burn. It requires
every current mint supply to equal the cache exactly and otherwise refuses
`ShadowSupplyMismatch`. A conflicting resolution still refuses from the
persisted record/evidence relation.

Resolution is otherwise claim-neutral: it does not mint, burn, transfer,
redeem, materialize, dematerialize, pay collateral, credit Position cash, or
change locked backing. It changes lifecycle/kernel phase and writes the sole
versioned resolution record:

- v2 stores a selected finite-payout index;
- v3 stores the native point-derived vector;
- v4 stores the native quantized-occupation vector plus archive/statistic
  bindings.

No versioned record stores or commits a claim-supply snapshot. Its conflict
identity is source/window/vector metadata, not holdings.

### 1.3 What later consumers do

The present live paths remain stronger than a claim-neutral Resolve requires:

- `Split`, `Merge`, `Materialize`, and `Dematerialize` observe and synchronize
  the complete mint vector. Materialize/dematerialize are Active-only at the
  kernel seam, so they refuse after resolution.
- recorded `RedeemInternal` receives the complete mint vector, synchronizes it,
  performs the exact v2/v3/v4 payout, then verifies zero mint delta and
  re-closes SupplyLedger against the kernel;
- `RedeemExternal` receives the complete vector, synchronizes it, burns the
  named bearer claim, pays exact collateral, re-observes every mint, requires
  one exact burn delta and zero deltas elsewhere, then persists the new cache.

Thus no currently implemented payout-moving path can turn a mint-free
resolution record into payment without first consulting authoritative mint
truth.

### 1.4 Hoard meaning

Resolution also moves no Hoard quantity. The relevant pooled relation is:

```text
H = L + C_cash + S
Q <= L
```

where `H` is the actual Hoard token amount, `L` is locked backing,
`C_cash` is aggregate Position cash, `S` is unowned direct-deposit surplus, and
`Q` is required collateral for claims. A direct Egg burn may reduce `Q` but
does not change `H`, `L`, cash, or surplus. That retained slack is conservative
overcollateralization, not a fee or sweep right. A direct Hoard token donation
raises only `H` and `S`. Claim-neutral resolution leaves both cases untouched.

## 2. The exact safety contract

The following conditions are sufficient for omitting the mint vector while
retaining SupplyLedger as a read-only closure fact.

### R1 — canonical sole mint authority

Every outcome mint is canonical, extension-free, and has exactly the Market
PDA as mint authority. No other program or permanent delegate can increase
`A_i`. Every program path that mints persists the matching cache/aggregate
increase atomically and verifies the exact Token-2022 delta.

This makes `A_i <= C_i` inductive. Without R1, omission is unsound.

### R2 — exact program-state closure

Before resolution, for every active outcome:

```text
T_i = I_i + C_i
```

All additions are checked and all inactive padding is zero. Keeping the
SupplyLedger account on the claim-neutral Resolve plane preserves this local
corruption check even though the account can become read-only.

### R3 — conservative cache

On every reachable state, `A_i <= C_i`. A stale cache may overestimate but must
never underestimate authoritative bearer supply. Holder burns preserve this
one-sided invariant.

### R4 — active solvency

Locked backing covers the active-phase requirement computed from the
conservative `T` vector.

For finite mode:

```text
L >= max_{p in immutable payout set} liability(T, p)
```

For derived mode:

```text
L >= max_i T_i
```

The latter is the exact supremum over admitted simplex weights, not a heuristic
buffer.

### R5 — resolution is strictly claim/value neutral

Resolution may write only lifecycle/kernel resolution fields and the canonical
resolution record. It must not change `I`, `C`, `A`, `T`, `L`, Position cash,
Hoard tokens, reservations, or entitlements. The selected payout must depend
only on immutable Terms and authenticated source/archive facts, never current
holdings.

### R6 — phase-change liability cannot rise

For v2, the selected payout is one member of the finite set whose maximum was
already covered in Active phase. For v3/v4, valid weights are nonnegative, sum
to denominator `D`, and therefore:

```text
ceil(sum_i T_i * w_i / D) <= max_i T_i
```

Changing Active to Resolved with the same conservative `T` cannot increase
required collateral.

### R7 — payout-moving consumers synchronize first

Before any later instruction burns or debits a claim, lowers locked backing,
credits cash, or transfers collateral, it must:

1. authenticate the complete canonical mint vector;
2. require cached closure;
3. require `A_i <= C_i` for every outcome;
4. lower `C` and `T` to current truth with checked arithmetic; and
5. run the kernel and exact Token-2022 pre/post delta checks atomically.

This condition may later be weakened only by a separately proved partial-sync
rule. The current live consumers already use the full-vector rule.

### R8 — resolution conflict identity excludes supply

Exact replay and conflict detection are decided solely from the canonical
resolution record and immutable evidence relation. Supply synchronization is
not allowed to become a second resolution identity. A burn after resolution
may change when the cache reconciles, but cannot turn one payout fact into
another.

### R9 — rollback and arithmetic remain fail-closed

Every synchronization and payout computes all checked additions,
multiplications, subtractions, and exact-lot divisibility before committing a
value movement, with Solana transaction rollback covering late CPI failures.
A mint-free Resolve adds no new arithmetic over claims.

## 3. Why the theorem holds

Let `T^c = I + C` be conservative totals and `T^a = I + A` actual totals. R3
gives `T^a <= T^c` componentwise. Payout weights are nonnegative, so liability
is monotone in each supply coordinate:

```text
liability(T^a, p) <= liability(T^c, p)
```

R4 and R6 therefore show that both the resolved conservative liability and the
actual liability remain at most `L`. R5 means Resolve changes none of those
quantities. A later direct burn lowers `A` again and cannot worsen the bound.
At the first payout-moving consumer, R7 lowers `C` and `T` to current truth;
monotonicity means this cannot create insolvency. The consumer then performs
the same payout and post-state checks it would have performed if Resolve had
synchronized earlier.

Consequently, full synchronization at Resolve and deferred synchronization at
the next consumer have the same successful economic result. Their intermediate
cache bytes and refusal timing differ.

## 4. The minimal counterexample and the hard STOP

Consider two worlds with identical Market, Terms, Hoard, kernel,
SupplyLedger, source/archive, and resolution accounts:

```text
world G: A_k = C_k
world B: A_k = C_k + 1
```

The only differing bytes are in omitted outcome mint `k`. A deterministic
mint-free Resolve receives identical inputs in both worlds and must return the
same result. It therefore cannot both accept world G and detect the impossible
increase in world B.

This is not repaired by observing a subset. If `k` is outside the subset, the
same indistinguishability argument applies. A partial vector proves only the
indices it reads and does not establish the whole-market conservative bound.

Therefore:

- if R1-R3 are accepted as inductive reachability invariants and deferred
  detection before the first payout is acceptable, claim-neutral Resolve is
  safe;
- if `Resolve` itself must locally attest `A_i <= C_i` against arbitrary
  corrupted account state, every mint (or a cryptographically equivalent
  authenticated aggregate maintained by Token-2022 itself) is necessary;
- a caller-supplied supply vector, client index, or unchecked commitment is not
  an equivalent fact and must not be admitted.

The model fault-injects world B. Current full observation refuses without
writes. Claim-neutral resolution records the payout, but both internal and
external redemption then re-read the full vector and refuse before any payout.
This is fail-closed value movement with later fault detection, not identical
admission behavior.

## 5. Candidate account-plane choices

| Choice | Mint accounts | SupplyLedger | What it proves locally | Disposition |
| --- | ---: | --- | --- | --- |
| Current | `n` read-only | writable | closure, current `A <= C`, zero Resolve mint delta | strongest refusal surface; expensive |
| Cache-only claim-neutral | `0` | read-only | closure and conservative-kernel solvency, relying on R1-R3 | **recommended optimization candidate** |
| Kernel-only claim-neutral | `0` | omitted | kernel self-invariant only; no `I + C = T` cross-check | **STOP for first cut** |
| Record-only | `0` | omitted, and potentially no Hoard | only evidence/record conflict; all solvency is inductive | **STOP absent a stronger end-to-end refinement** |

The cache-only option removes no semantic owner. SupplyLedger remains the
decomposition owner and the kernel remains the aggregate owner, with their
closure checked on Resolve. The only omitted state is authoritative mint truth
that a claim-neutral transition does not consume.

Omitting SupplyLedger too could still be economically safe on perfectly
reachable histories, but it would stop checking a cheap program-owned
cross-account invariant. The account saved is not worth combining two trust
cuts in the first optimization.

## 6. Structural savings and honest CU claim

For `n = market.outcome_count`:

| Path | Current accounts | Cache-only accounts | Saved |
| --- | ---: | ---: | ---: |
| legacy-buffer finite-preset / native point | `11 + n` | `11` | `n` read-only mint roles |
| archive-direct V2 degree-zero / occupation | `10 + n` | `10` | `n` read-only mint roles |

At the maximum `n = 16`, point resolution falls from 27 to 11 accounts and
archive-direct resolution from 26 to 10. It also removes, from Resolve only:

- two canonical mint-PDA derivations per outcome (pre and post);
- two complete Token-2022 mint decodes/policy admissions per outcome;
- the synchronization loop and the post-state zero-delta loop;
- SupplyLedger writes caused solely by recognizing direct burns; and
- `n` read locks and account metas from the transaction.

The raw uncompressed public-key payload represented by 16 ordinary account
keys is 512 bytes, although the actual transaction saving depends on message
version and address lookup tables. No exact serialized-byte claim is made here.

No exact CU saving is claimed without rebuilding and measuring the live SBF
artifact. The removed work includes PDA derivation syscalls and Token-2022 TLV
admission, so the saving should be material, especially at large `n`; its value
must be reported only as bank-measured before/after rows for each of v2, v3 d1-
d3, and v4 d1-d3 against one attributed source boundary. The model pins the
structural count, not runtime CU.

The failure surface removed from Resolve includes incomplete/out-of-order mint
suffixes, wrong mint PDA/owner/mutability/authority/decimals/extensions,
unaccounted increases, pre/post mint changes, and late supply synchronization
failure. These checks are not deleted protocol-wide: R7 retains them on every
payout-moving consumer. Current exact replay's burn-sensitive
`ShadowSupplyMismatch` also disappears; exact replay becomes purely a
resolution-record question.

## 7. Executable adversarial evidence

`research/claim-neutral-resolution/src/lib.rs` is dependency-free and uses
checked integers and transactional clone/commit transitions. Its tests cover:

- stale cache overestimates from direct burns before Resolve, followed by
  state convergence and identical internal payout at the next full-sync
  consumer;
- arbitrary burns after Resolve, exact replay, and independent resolution
  conflict detection;
- the unaccounted-increase indistinguishability counterexample and deferred
  refusal before internal or external payout;
- partial-vector observation proving only its selected indices;
- v2, v3, and v4 mode/record identity and conflicting replay;
- materialize/dematerialize conservation before Resolve and phase refusal
  after Resolve;
- exact internal and external redemption, retained burn overcollateralization,
  direct Hoard donations, and pooled custody closure;
- malformed cached-addition overflow, impossible-increase classification, and
  atomic refusal; and
- a bounded depth-five state-machine exploration over burns, bridge moves,
  resolution, both redemption modes, and donations, checking reachability after
  every successful transition and byte-for-byte model state on every refusal.

The footprint test pins the maximum-width 27-to-11 and 26-to-10 account changes,
plus 32 removed mint PDA derivations and 32 removed mint admissions.

This is executable design evidence, not Solana runtime evidence, a formal proof,
or permission to modify the live ABI.

## 8. Implementation gates if this optimization is authorized

1. Introduce an explicitly versioned Resolve account plane. Do not make one
   decoder accept both suffix shapes by guessing from account count.
2. Retain SupplyLedger read-only and preserve exact `I + C = T` closure before
   the semantic gate. Keep Market, Hoard, kernel, Terms, Resolution, Feed,
   SourceSpec, SourceArchive, and the point projection where the selected mode
   still needs them.
3. Remove only `observe_outcome_mints`, Resolve-time
   `synchronize_external_truth`, exact-repeat cache equality, the post-observe,
   and zero-delta check. Do not weaken source/archive, payout, lifecycle,
   collateral, or record-conflict checks.
4. Keep full-vector synchronization on every payout-moving consumer. Add a
   static account-plane audit that no newly routed consumer bypasses it.
5. Add real-SBF cases for burns before/after v2/v3/v4 Resolve, exact and
   conflicting replay, impossible-increase fault injection, both redemption
   modes, donation surplus, and late rollback.
6. Prove or exhaustively bridge the Rust claim that resolution never raises the
   conservative collateral requirement for both basis modes and that lowering
   supply after resolution cannot raise it.
7. Measure exact account counts, ELF digest/size, stack diagnostics, CU for
   first resolution and replay, and consumer CU. Do not extrapolate model
   counts into a CU claim.
8. Update `TOKEN2022_EXTERNAL_TRUTH_V1.md`, `PROTOCOL.md`, SBF module comments,
   client builders, harnesses, evidence matrix, and handoff truth in one
   coherent commit. The present documents correctly describe the present full
   vector and must not be left half-migrated.

Release remains stopped if any payout-moving consumer can be reached without
current mint synchronization, if an outcome mint admits another minting
authority or extension, if SupplyLedger/kernel closure can diverge through a
public path, or if immediate impossible-increase detection at resolution is
retained as a product requirement.
