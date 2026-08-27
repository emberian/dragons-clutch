# Failure payout and terminal value decision V1

Status: **DECIDED FOR NEW RESEARCH PROFILE / MODEL-ONLY / HOST-TESTED**
(2026-08-19). The executable falsifier is
[`research/failure-payout-v1`](../../research/failure-payout-v1/). This decision
changes no live ABI, SBF program, Token-2022 mint, existing Market, source
release, or deployment claim.

## Decision

Freeze the new-market policy as `EvidenceOnlyRecoveryV1`:

1. **There is no numeric data-failure payout.** A payout vector is recorded only
   from the same Terms-admitted authenticated evidence relation used for normal
   resolution. If the relation has not selected one vector, the market cannot
   resolve.
2. A missing required observation moves the market to
   `DEGRADED_RECOVERABLE`: stop new order exposure, preserve ordinary
   Token-2022 transfers, keep exact complete-set merge available, and execute a
   finite permissionless repair schedule from its independently prepaid SOL
   compartment.
3. When that finite schedule ends without valid evidence, move to
   `RECOVERY_DORMANT`. Transfer unused failure residue from the dedicated SOL
   reserve to the canonical SDK incinerator and verify the exact reserve delta.
   It does not return to a creator, resolver, subscriber, claimant, executor,
   or treasury. A later caller-funded submission of valid evidence may still
   resolve the market; no work payment is promised after the finite reserve
   ends.
4. Low volume is not a resolution input. It may stop new exposure, but it does
   not change claims, release backing, reduce repair funding, or select a
   fallback vector.
5. Time does not prove abandonment. Every nonzero internal claim, current
   Token-2022 outcome-mint supply, owner cash balance, or numerator credit stays
   live until an authorized burn, merge, redemption, transfer, or withdrawal
   changes it.
6. New native markets using this policy version use lot-scaled bearer units.
   One Token-2022 outcome
   token atom represents a creation-time universal raw-claim lot `L` with
   `D | L`; every claim-separating path preserves that lot. The conservative
   first profile uses `L = D`. Existing raw-unit bearer mints are not relabeled
   and remain exact-or-refuse/permanent until separately migrated.
7. This profile creates no persistent fractional credits. If another profile
   imports them, their aggregate numerator remains a first-class liability and
   any nonzero credit prevents terminal disposal. A sub-atom is not a sweepable
   remainder.
8. After all economic owners and booked work are zero, remaining whole Hoard
   collateral—direct donations plus holder-burn forfeiture—is destroyed under
   an immutable creation-time `BURN` disposition. It is never a repair fund,
   rent fund, bounty, refund, rebate, fee, or treasury asset. A Realm whose
   admitted collateral profile cannot support an authenticated exact burn must
   refuse this market version rather than substitute an interested recipient.

This policy is deliberately not a promise of total retirement. One abandoned
legacy sub-lot, a nonzero numerator credit, a permanent source dependency, or a
nonzero bearer mint can keep the declared account set live forever. That is the
non-confiscatory result, not stranded value disguised as cleanup.

## Why no fixed failure vector can be neutral

Let `V(e)` be the set of payout vectors still possible under frozen
authenticated evidence `e`. Every vector has nonnegative integer weights that
sum to the common denominator `D`. Suppose a failure rule selects one fixed
vector `f(e)` before the evidence relation has selected a singleton.

For any possible successful vector `v in V(e)` with `f != v`, equal sums imply:

```text
exists i: f_i > v_i
exists j: f_j < v_j
```

Outcome `i` gains from inducing failure relative to completion `v`; outcome
`j` loses value without evidence selecting that loss. Therefore a fixed
fallback is distribution-neutral against every possible completion if and
only if every vector in `V(e)` equals it. In that singleton case it is ordinary
evidence resolution, not a failure rule.

The executable model exhausts every pair of two-outcome integer-simplex
vectors through `D = 32` and checks that every unequal pair has both a gainer
and a loser. This is a bounded falsifier, not a universal formal proof, but the
general argument is the exact equal-sum identity above.

Evidence may conclusively eliminate outcomes under a source-specific theorem.
That narrows `V(e)`; it does not justify equalizing the remaining outcomes. If
the narrowed set is still not a singleton, the same argument applies.

## Operatorless adversarial starting states

The policy is chosen against these states, not an expected-volume forecast:

1. **Zero-volume disappearance:** immediately after claim creation, every
   maintainer disappears and no later order pays a fee. Previously admitted
   observation/repair work still has its SOL reserve; claim backing is
   unchanged.
2. **Near-certain outcome attack:** the market prices outcome `0` near one. An
   attacker buys cheap tail Eggs and can delay a publisher or censor repair.
   Equal, `INVALID_DATA`, and equal-compatible fallbacks transfer value to the
   attack. Evidence-only recovery creates no tail payout.
3. **Permanent common-mode source loss:** all finite paid attempts fail. The
   reserve terminates exactly, the market becomes dormant, and claims remain.
   The protocol does not manufacture an inclusion guarantee or a price fact.
4. **Congestion above the admitted maximum:** a keeper payment is inadequate
   by one lamport. The attempt refuses or fails to land; it may not debit the
   Hoard, claim cash, rent principal, future fees, or another market.
5. **Bearer burn between protocol calls:** an ordinary holder burns Token-2022
   Eggs. Actual mint supply falls below its program cache. The next full-vector
   synchronization recognizes forfeiture; no claimant credit or refund is
   invented and locked backing remains conservative.
6. **Impossible bearer increase:** current Token-2022 mint supply exceeds the
   last program-observed supply. Resolution, redemption, and terminality refuse
   before value movement. A cache or indexer cannot override mint truth.
7. **One abandoned holder:** every account is otherwise empty, but one outcome
   mint has one token atom. No deadline, rent pressure, or terminal authority
   may extinguish it.
8. **Fractional-credit tail:** all raw claims are gone and the global credit
   numerator is `0 < K < D`. Paying zero confiscates; paying one chooses an
   overpaid owner; a sink confiscates. The credit and enough backing remain.
9. **Donation/prefund capture:** an arbitrary actor prefunds a predictable PDA
   or sends collateral to the Hoard. No later payer, creator, closer, or keeper
   becomes its owner. It follows only the immutable donation/terminal rule.
10. **Collateral-price collapse:** atom solvency remains exact while external
    purchasing power collapses. SOL work is still separately funded; no
    collateral-to-SOL swap or reward-token price assumption enters admission.

## Conservation law

For outcome `i`:

```text
I_i = aggregate program-owned raw claims
E_i = current authoritative Token-2022 token supply
L_i = raw claims represented by one bearer token atom
T_i = I_i + L_i * E_i
```

The first native profile uses one common `L_i = L`, with `D | L`. The runtime
may cache observed mint supply `C_i`, but successful payout/terminal
transitions authenticate current `E_i`; `C_i` never becomes bearer truth.

For Hoard collateral atoms:

```text
H = L_locked + P_cash + S_direct
0 <= R_reserved <= P_cash
```

`R_reserved` is a subset of cash, not another Hoard term. Holder burns do not
change this identity. They reduce required liability while `L_locked` stays
fixed, creating conservative slack inside locked backing.

Before resolution, with admitted payout family `V`:

```text
Q_active(T) = max(v in V) ceil(sum_i T_i*v_i / D)
L_locked >= Q_active(T)
```

For the full native integer simplex, `Q_active(T) = max_i T_i`. In degraded
and dormant recovery, the same bound remains. No fallback releases the
difference.

After evidence selects weights `w` and persistent credit numerators total `K`:

```text
N = sum_i T_i*w_i + K
D * L_locked >= N
```

For claimant residue `r`, burning `q` raw claims of outcome `i` is exact only
through:

```text
r + q*w_i = D*payout + r'
T_i'       = T_i - q
K'         = K + r' - r
L_locked'  = L_locked - payout
```

Both sides of the solvency inequality fall by `D*payout`. Under the selected
lot profile `r = r' = K = 0` and `D | q*w_i` at every permitted redemption.
Under a credit profile, `K` is liability, not dust.

The repair equation is disjoint:

```text
B_repair = reserve_account_balance
         + keeper_paid
         + success_payer_refund
         + neutral_failure_incinerated
```

`reserve_account_balance = pending_work` while the reservation is live. There
is no `H`, `L_locked`, claim, fee, future volume, token price, or future
subscriber term in this equation. On failure the payer refund is zero and the
remaining balance is transferred exactly to `solana_sdk_ids::incinerator::ID`.
On successful evidence repair, unused reserve may return to its recorded payer.

## Candidate comparison

| candidate | solvency | distribution/incentive | decision |
| --- | --- | --- | --- |
| Equal across all outcomes | Solvent only if frozen in the active maximum | Cheap tails gain when a likely outcome can be denied | **Reject** |
| Dedicated `INVALID_DATA` Egg | Solvent if prefunded | Makes source failure directly tradeable and rewards inducing it | **Reject** |
| Equal among evidence-compatible outcomes | Can be solvent | Exclusion needs a source theorem; equalization among survivors is still arbitrary unless singleton | **Reject as a neutral rule** |
| Last observation / last-good mark | Can be solvent if frozen at creation | Stalling at a favorable observation changes payout; semantics are provider-specific | **Reject as generic R4 rule**; a future source-specific product may propose it openly |
| Venue price, TWAP, or auction mark | Can be bounded | Circular in low volume and manipulable precisely when the source failed | **Reject** |
| Creator, resolver, keeper, governance, or treasury choice | Depends on discretion | Restores an operator and gives an interested party payout authority | **Reject** |
| Pro-rata refund of Hoard | Generally insolvent or confiscatory | Hoard contains owner cash, backing, donations, and burn slack with different owners | **Reject** |
| Expire inactive claims and pay remaining holders/cleanup | May balance after confiscation | Time is not burn consent; creates an abandonment-grief market | **Reject** |
| Evidence-only recoverable dormancy | Keeps active solvency bound | No failure-selected redistribution; capital may remain locked indefinitely | **Select** |

The selected rule does not claim that denial has no economic harm. An attacker
can still delay liquidity and capital release. It removes the direct
state-contingent payout transfer that equalization creates; common-mode
exposure caps and source-security tiers remain necessary.

## Abandoned claims and bearer truth

There is no onchain test for “abandoned.” The only liability-reducing facts are:

- successful complete-set merge;
- exact internal or bearer redemption;
- authenticated owner burn/transfer that actually changes the relevant
  balance; and
- ordinary Token-2022 bearer burn, observed as lower authoritative mint supply
  and classified as forfeiture.

External supply is the current canonical Token-2022 mint supply, not a Position
shadow, client snapshot, indexer, or last-observed cache. Every terminal check
must receive the complete canonical mint vector or an equivalent authenticated
Token-2022 aggregate that does not exist today. `MintCloseAuthority` is useful
only after authoritative zero; it is not authority to erase supply.

Ordinary holder burn has this effect after synchronization:

```text
E_i'       = E_i - b
L_locked'  = L_locked
P_cash'    = P_cash
claimant credit = unchanged
```

It is unilateral forfeiture. Its retained value cannot pay the synchronizer,
repairer, creator, remaining holders, rent, or treasury. Only the terminal
burn rule may destroy it after every remaining owner is zero.

## Fractional native decision

`EvidenceOnlyRecoveryV1` selects **lot-scaled bearer units**, not numerator
credits:

```text
one bearer token atom = L raw claims
D | L                     # first conservative profile: L = D
```

Every Split, Merge, internal transfer, reservation, fill leg, Materialize,
Dematerialize, and redemption that can separate claims must preserve the
creation-time lot. Token-2022 may then transfer one token atom freely because
the token atom itself is a whole economic lot. Token decimals do not provide
this mapping; Market Terms, supply accounting, SDK display, and exact CPI delta
checks must all name it.

This is a new-market encoding. Existing outcome mints whose atom represents one
raw claim are not silently rescaled. They remain exact-or-refuse, and an
abandoned sub-lot can keep them permanent.

Persistent numerator credits remain a coherent conservation mechanism for a
different product, but they do not solve terminality with indivisible
collateral. If:

```text
K = D*A + r, 0 <= r < D
```

then owners may voluntarily aggregate enough same-domain credits to withdraw
the `A` whole atoms. If `r > 0`, no exact terminal payout exists without finer
collateral, explicit owner forfeiture, or an independently capitalized subsidy
plus a separately justified allocation rule. The Hoard cannot provide that
subsidy. This V1 therefore creates no credit accounts and treats every imported
nonzero `K` as a terminal STOP.

## Terminal disposition and exact declared residue

Terminal collateral burn requires all of the following in one authenticated
generation:

```text
all reservations/entitlements consumed or released
P_cash = 0 and R_reserved = 0
I_i = 0 for every outcome
current Token-2022 E_i = 0 for every outcome
K = 0 and no live credit account
all mandatory work reservations terminal
all dependent source/archive references released
```

Only then may the Hoard PDA burn exactly the remaining whole collateral amount
and verify exact Hoard and collateral-mint post-state. No human, treasury,
creator, keeper, resolver, claimant, rent recipient, or protocol token holder
receives it. Account rent follows its separately persisted principal owner;
unowned lamport donations follow their separately frozen neutral disposition.
Outcome mints close only with their creation-time `MintCloseAuthority` after
authoritative zero. The compact replay/generation tombstone remains permanent.

Burning collateral can have external monetary effects and some collateral
profiles may forbid or complicate it. Therefore `BURN` is an admitted Realm
capability, not an Eggcrate assumption. If exact owner-authorized burn and
post-state verification are unavailable, this profile's value admission is a
STOP. Substituting a spendable token account is not equivalent.

## Minimum onchain authorities for promotion

1. Immutable Market `failure_policy = EvidenceOnlyRecoveryV1`, source/profile
   identity, payout denominator, native raw-to-token lot, generation, and
   `terminal_disposition = BURN`.
2. An exhaustive phase machine with `ACTIVE`, `DEGRADED_RECOVERABLE`,
   `RECOVERY_DORMANT`, `RESOLVED`, and terminal/tombstone states. No generic
   “admin resolve” branch.
3. A separately funded repair reservation with immutable payer, exact deposit,
   accepted-work counter/generation, keeper-paid total, success refund,
   canonical SDK-incinerator failure transfer, donation ledger, authenticated
   deadline, and replay protection.
4. Current whole-vector Token-2022 outcome-mint authentication for every claim
   or terminal value transition; SupplyLedger remains internal aggregate/cache,
   never bearer authority.
5. Exact Hoard compartments for locked backing, aggregate owner cash,
   reservations as a cash subset, and direct donations. No instruction takes a
   work/rent/fee destination from this account.
6. If any credit profile exists, immutable `(Market,D,generation,claimant)`
   credit keys and one checked market aggregate `K`. Terminal checks cannot scan
   users or infer zero from missing accounts.
7. Creation-time Token-2022 `MintCloseAuthority` for new outcome mints and a
   separately authenticated Hoard burn authority/collateral profile. Legacy
   mints lacking close authority remain permanent infrastructure.
8. Exact persisted rent-principal owner, independent payer deposit, monotone
   donation ledger, close recipient, and generation identity for every
   refundable account. Hoard value is never rent principal.
9. Source/archive reference ownership through the last unresolved or retryable
   market; dormant recovery cannot outlive the evidence objects it needs.
10. A permanent generation/consumed-ID replay tombstone funded independently at
    creation. Refunded Market rent cannot recreate it.

These are minimum semantic owners, not a proposed account packing. Combining
fields is acceptable only if it preserves one canonical owner and the same
transactional checks.

## Promotion falsifiers

Any one of these keeps runtime promotion stopped:

1. A trace records a payout vector without the Terms-admitted evidence relation
   selecting it.
2. Low volume, elapsed time, failed attempts, caller identity, or reserve
   exhaustion changes a claim weight or extinguishes a claim.
3. Any repair/observation/finalization/cleanup payment succeeds when its SOL
   compartment is one lamport short, or debits Hoard/cash/rent/future fees.
4. Failure residue reaches a creator, resolver, keeper beyond accepted work,
   claimant, maker, executor, treasury, or subscriber refund.
5. A claim-separating path creates a raw quantity not divisible by the frozen
   lot, or a bearer token atom is displayed/accounted as a different raw lot.
6. A payout/terminal path trusts cached external supply when current canonical
   Token-2022 mint supply differs, or accepts an impossible supply increase.
7. Direct bearer burn credits any person, reduces owner cash, or unlocks Hoard
   value to a nonterminal destination.
8. A nonzero internal claim, bearer mint supply, cash balance, reservation, or
   numerator credit can be expired, swept, or closed.
9. A credit state with `K mod D != 0` is called fully settled without finer
   units, explicit holder forfeiture, or a separately capitalized and frozen
   allocation rule.
10. Terminal collateral leaves the Hoard before every owner/work/source
    dependency is zero, or reaches a spendable recipient instead of the admitted
    exact burn.
11. Rent refund identity is inferred from a closer/signer or captures a prior
    prefund/donation.
12. Closing an outcome mint relies on cached supply, lacks creation-time
    `MintCloseAuthority`, or permits stale generation recreation after refund.

## Executed model evidence and boundary

The model is safe `no_std`, no-allocation, float-free Rust with fixed arrays and
checked arithmetic. Its hostile tests cover:

- the bounded fixed-fallback redistribution theorem through `D = 32`;
- zero-volume/team-disappearance repair exhaustion with invariant Hoard bytes
  and an exact separately conserved SOL reserve;
- one-lamport repair underfunding and byte-for-byte refusal atomicity;
- premature-window-close refusal, active-release exposure closure, and dormant
  recovery from later valid evidence without budget resurrection;
- exact lot materialization and fractional resolved bearer redemption;
- ordinary holder burn, stale-cache synchronization, and retained backing;
- forged upward/downward Token-2022 vectors and an actual impossible increase
  before resolution;
- indefinite abandoned bearer claims;
- an explicit terminal fractional-credit STOP; and
- exact terminal destruction of direct donations and holder-burn backing only
  after all owners, work, reservation, source, and refundable-account gates are
  zero.

Passing those tests proves only the model's bounded arithmetic and phase
relations. Promotion still requires a versioned ABI, hostile-byte parsers,
source-specific evidence theorem, lot preservation across the actual venue,
real Token-2022 CPI/post-state evidence, measured repair/terminal SBF paths,
rent and source-reference integration, formal refinement, and a new exact
artifact/evidence seal.
