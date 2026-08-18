# Multi-position aggregate closure (CLO-DELTA-V1)

Status: PROPOSED and implemented in the offline reference adapter
(`programs/solana-reference`); not an SVM program, deployment artifact, or
chain-readiness claim. This design discharges the *representational* half of
`SOLANA_REFERENCE_ADAPTER.md` obligation 11 — a closure scheme that admits many
positions without scanning them — and names exactly which halves stay with the
runtime obligations (1, 2, 3, 9).

## 1. The problem

The reference adapter enforced a closed single-position model. Before and
after every transition, for every active outcome `o`:

```text
E1  supply.internal_supply[o] + supply.external_supply[o] == kernel.total_supply[o]
E2  position.internal[o]                                  == supply.internal_supply[o]
E3  external.balance[o]                                   == supply.external_supply[o]
```

`E1` is the `SupplyLedgerAccount`'s own two-term closure and holds for any
number of positions. `E2`/`E3` identify the market-wide terms with the one
presented position, which is the strongest honest linkage available to a check
that sees one position — and it is also exactly what makes a second position
unrepresentable: any state where two owners both hold claims violates `E2` and
refuses `AggregateClosureMismatch`.

A real market has many positions. The requirement (obligation 11): each
owner's transition must update the one market aggregate exactly once, and
closure must prove all internal and external balances are represented without
scanning an unbounded set.

## 2. Schemes considered

### (a) Delta-accounting — PROPOSED

The aggregate (the `SupplyLedgerAccount` two-term ledger, joined to
`KernelAccount.total_supply` by `E1`) is the only counted truth. Every
transition touches exactly one position triple (position, external shadow,
replay) plus the market-wide accounts, and applies to the ledger *exactly the
delta it applies to the position*. Closure then holds by induction over the
transition history, not by a global scan:

- **Base case**: a position enters the system provably zero.
- **Inductive step**: one transition changes one position and the ledger by
  equal per-outcome deltas, so the sum over all positions changes by the same
  amount the ledger changes.

No transition ever needs to see a second position. The per-transition checks
are all expressible over the accounts the adapter already decodes, so this
lands as an extension of the existing check structure, not a rewrite.

### (b) Per-position commitment accumulators — rejected for V1

The ledger would hold a homomorphic accumulator `C = Σ_p commit(balances_p)`;
each transition updates `C` by `commit(post) − commit(pre)`; closure is an
opening proof. Rejected because:

- On SVM every balance in this system is public account data. A binding,
  non-hiding homomorphic commitment over public vectors degenerates to the
  componentwise sum — which is precisely scheme (a). The machinery adds an
  algebraic dependency and buys nothing until balances are private, which is
  out of scope for every crate here.
- This crate deliberately owns **no hash or group primitive** (the same
  argument that makes `WindowId` a trusted binding with a published preimage).
  An accumulator would force one into the TCB now, for zero present security.

Scheme (a) *is* scheme (b) with the identity commitment; if private balances
ever arrive, (b) is the upgrade path and the delta discipline transfers
verbatim.

### (c) Epoch-based reconciliation windows — rejected as the primary invariant

Periodically enumerate all positions (bounded pages per epoch, the
`BATCH_RELATION_V1_DESIGN.md` order-page pattern) and re-prove
`Σ_p == ledger`. Rejected because:

- It converts a per-transition invariant into an eventually-checked one:
  between reconciliations a forged state is live and spendable, which is the
  opposite of the fail-closed discipline.
- The scan it was supposed to avoid comes back, amortized: position count is
  unbounded, so either epochs stretch or page counts grow without bound, and
  the reconciler needs an enumerable position registry — per-position state in
  consensus, which is exactly what the ledger design refuses to hold.

Reconciliation can return later as *audit* defense-in-depth on top of (a); it
is not the closure mechanism.

## 3. The invariant, precisely

Fix a market `M`, its supply ledger `L` (`SupplyLedgerAccount`), its kernel
aggregate `K` (`KernelAccount`), and the set `P` of position triples
(`PositionAccount`, `ExternalAccount`, `ReplayAccount`) initialized under `M`
in `L`'s accounting era. For every active outcome `o < M.outcome_count`:

```text
I1 (two-term aggregate):  L.internal_supply[o] + L.external_supply[o] == K.total_supply[o]
I2 (represented balances):     Σ_{p ∈ P} p.position.internal[o]  == L.internal_supply[o]
                          and  Σ_{p ∈ P} p.external.balances[o]  == L.external_supply[o]
I3 (per-position bound):  for each p ∈ P:  p.position.internal[o] <= L.internal_supply[o]
                          and              p.external.balances[o] <= L.external_supply[o]
```

`I3` follows from `I2` and non-negativity; it is stated separately because it
is the part of `I2` a single transition can *check* about the one triple it
sees. `I2` itself is a statement about all reachable states — a theorem about
histories, maintained inductively, never scanned.

### Per-transition obligations (the checked set)

- **C0 — initialization (base case)**: a position triple is admitted to the
  system only in the provably-zero state: all internal balances zero, all
  external shadow balances zero, position cash and reserved cash zero,
  `close_state == 0`, replay `sequence == 0`, and the triple mutually bound
  (same market, same owner, same generation across position/external/replay).
  New entry point `validate_position_init`; refusal class
  `NonEmptyInitialization` (the same class market init already uses for its
  founding triple).
- **C1 — two-term closure, pre and post**: `I1` checked over the decoded
  ledger and kernel accounts before the transition and again over the
  post-state before encoding. This is `E1`, retained unchanged. Refusal:
  `AggregateClosureMismatch` (or `Arithmetic` if the two-term sum itself
  overflows).
- **C2 — representation bound, pre and post**: `I3` for the presented triple:
  `position.internal[o] <= L.internal_supply[o]` and
  `external.balances[o] <= L.external_supply[o]`. This **replaces** `E2`/`E3`.
  Refusal: `AggregateClosureMismatch`.
- **C3 — delta equality (the inductive step)**: the ledger post-state is
  *computed* as `L' = L − pre(position) + post(position)` per term, per
  outcome, with checked arithmetic (`apply_position_delta_to_ledger`), never
  overwritten with the position. Together with C1-pre and C1-post this yields
  a cross-check with teeth: the kernel's own supply effect
  (`K.total_supply` delta) must equal the position delta, because
  `C1-post` fails otherwise. A kernel transition whose aggregate effect ever
  diverged from its per-position effect would refuse rather than corrupt the
  ledger.

### Why the conjunction is closure

If every state in a history satisfies C1/C2 locally, and the history starts
from market init (all sums zero — `NonEmptyInitialization` already enforces
this) with every position entering through C0, and every step is a C3
transition, then `I2` holds at every state, by induction. The adapter checks
the base cases and the step; **reachability** — the guarantee that a position
account's bytes cannot exist except through this history — is exactly what
the offline model cannot supply and the runtime must:

- one live account per logical role: PDA derivation, obligation 1;
- only this program writes them: owner/writable authentication, obligation 2;
- no double-apply, no resurrection of closed-account bytes: obligation 3/9.

A forged pre-state that satisfies C1/C2 but violates `I2` (e.g. two positions
"both" holding the same ledger-counted claims) is locally undetectable by
construction — no per-transition check over one position can decide it — and
is precisely the state the runtime obligations exist to make unpresentable.
The design names this residue instead of pretending a local check covers it.

### "Exactly once" per transition

Each `apply` consumes one position triple plus the single market-wide ledger,
kernel, and hoard, and writes each of them exactly once. The ledger is the
serialization point: on SVM it is one writable account, so the runtime's
writable-account lock orders all concurrent position transitions of a market;
each serialized transition preserves the invariant, so any interleaving does.
Offline, each `apply` call is atomic (no output on any refusal).

## 4. Generations and eras

The single-position model identified `position.generation` with
`supply.generation` (`validate_links`). That identification cannot survive
multiple positions: each position bumps its own generation at close/reopen,
while the ledger has one generation. CLO-DELTA-V1 decouples them:

- **`position.generation`** is the per-position close/reopen era. It stays
  bound to the triple: `external.position_generation` and
  `replay.position_generation` must equal it (`validate_links`, unchanged).
  A reopened position is a **C0 event**: the new triple must be provably zero
  at the new generation, so stale balances cannot resurrect through reopen,
  and the replay sequence restarts namespaced by the new generation, so old
  signed requests do not replay.
- **`supply.generation`** is the market accounting era. The reference model
  admits exactly one era per ledger lifetime: no instruction writes it, so it
  is structurally constant across every transition, and the closure induction
  is defined within one era. The cross-identification with the position is
  dropped, and nothing per-transition replaces it, because era membership of
  a position is not decidable from the presented accounts — it is an
  initialization fact.
- **SVM rule proposed**: derive the ledger PDA from the market with **no
  close path** — one market, one ledger account, one era, for the market's
  whole lifetime. Then an era bump is structurally impossible and no
  per-position era stamp is needed. If a future design ever must close and
  re-create a ledger, it must re-establish the base case (bar every triple of
  the dead era, restart sums at zero), which requires an era stamp in the
  position account or its PDA seeds — a layout revision, recorded here as the
  cost of that choice, not defaulted into.

What the old identification actually protected — a stale ledger presented
beside a reopened position — is on SVM the runtime's single-live-account
guarantee (there is only one ledger account to present), and offline was
never enforceable anyway, since the caller supplies all bytes.

## 5. Refusal set

| Forged state | Catching check | Refusal |
| --- | --- | --- |
| position triple enters with nonzero internal, external, cash, reserved cash, sequence, or non-open close state | C0 | `NonEmptyInitialization` |
| position claims exceeding the ledger's internal term (counterfeit claims absent from aggregate) | C2 pre | `AggregateClosureMismatch` |
| external shadow exceeding the ledger's external term (forged external mint / ledger term burned without a position debit) | C2 pre | `AggregateClosureMismatch` |
| ledger terms not summing to the kernel aggregate (tampered ledger or kernel) | C1 pre | `AggregateClosureMismatch` |
| kernel transition whose aggregate supply effect diverges from its position effect | C1 post after C3 | `AggregateClosureMismatch` |
| two-term ledger sum unrepresentable in `u64` | C1 | `Arithmetic` |
| reopened position presented with a stale-generation external or replay account | triple binding (`validate_links`) | `MismatchedState` |
| replayed request against a reopened (sequence-restarted) triple | replay discipline | `Replay` |
| position/external/replay/supply keys aliased to any other role or the actor | `validate_metadata` / `validate_evidence_metadata` | `AccountAlias` |
| one owner's triple presented with another owner's position (owner mismatch inside the triple) | `validate_links` | `MismatchedState` |
| substituted account keys against the trusted bindings | `validate_metadata` | `WrongAccountKey` |

Accepted-and-conservative (not refusals, named deliberately):

- **Ledger over-counting** (`Σ_p < L` in some outcome — e.g. a position closed
  out-of-model while its balances stayed counted, or any donation-shaped
  residue): every check passes, the surplus claims' collateral stays locked in
  the Hoard, and nobody can spend them. Over-counting is the safe direction;
  the checks are one-sided exactly so that the *dangerous* direction
  (`Σ_p > L`, claims not backed by the aggregate) is the refused one.
- **Direct external burns** cannot occur in the reference (the shadow account
  has no path but `Materialize`/`Dematerialize`); on a real token, a holder
  burning bearer claims out-of-band produces ledger over-counting — the
  conservative direction — and the exact token-conservation question is
  obligation 6, not closure.

Named residue (locally undetectable, runtime-owned): a pre-state satisfying
C1/C2 whose position balances were never credited by any history — e.g. a
fabricated position account claiming claims that in truth belong to another
position. Unreachable through this adapter (C0 is the only entry and C3 the
only step); unpresentable on SVM under obligations 1–3/9. The offline adapter
checks histories' steps, not the runtime's account integrity.

## 6. Adversarial test list (implemented)

| Test | What it pins |
| --- | --- |
| `multi_position_lifecycle_tracks_ledger_sums` | two positions with distinct owners, keys, bumps, and *different* generations split, materialize, dematerialize, resolve, and both redeem; the ledger equals the componentwise sum of both triples at every step (test-side scan of the known set), the kernel aggregate equals the two-term sum, and the market drains to zero exactly |
| `position_init_forgery_refuses` | a provably-zero second triple validates; each nonzero field (internal, external, cash, reserved cash, sequence, close state) refuses `NonEmptyInitialization`; init claims exceeding the ledger refuse `AggregateClosureMismatch`; a resolved market refuses new positions |
| `generation_replay_after_close_reopen_refuses` | a reopened (generation-bumped) position with a stale-generation external or replay refuses `MismatchedState`; the reopened triple restarts at sequence zero and a stale sequence refuses `Replay`; reopen with surviving balances refuses `NonEmptyInitialization` |
| `aliased_position_keys_refuse` | a second owner's triple aliased onto the first's position key refuses (`WrongAccountKey` against trusted bindings, `AccountAlias` on shared role keys and on an actor-account alias, `WrongBump` on a foreign position's stored bump, `MismatchedState` on a bump-forged cross-owner triple) |
| `donation_and_direct_burn_accounting_is_one_sided` | ledger over-counting (unpresented positions' claims) is accepted and transitions stay sum-correct; position claims exceeding the internal term refuse; external shadow exceeding the external term refuses |
| `concurrent_same_slot_interleavings_commute_on_the_ledger` | both serialization orders of two owners' same-slot transitions yield the identical ledger post-state and preserve the sums; per-position replay sequences refuse cross-order replays; a stale ledger presented with balances it cannot cover refuses `AggregateClosureMismatch` |

Retained regressions: the counterfeit-claim counterexample (internal 1,
aggregate 0, `Materialize(0, 1)` refuses `AggregateClosureMismatch` with no
post-state) is caught by C2 exactly as it was by E2, and all twenty
pre-existing tests — including every exact byte vector — pass unchanged,
because on every single-position state where `E2`/`E3` held, C3's delta update
and the old overwrite produce byte-identical ledgers.

## 7. Mapping onto the existing adapter

| Existing structure | CLO-DELTA-V1 change |
| --- | --- |
| `validate_aggregate_closure` (E1+E2+E3, called pre and post) | same name, same call sites: now checks C1 (E1 unchanged) + C2 (bounds in place of the E2/E3 equalities) |
| ledger write-back in `apply_inner` (`supply.internal_supply = pure_position.internal`, the E2 overwrite) | `apply_position_delta_to_ledger`: checked `L − pre + post` per term per outcome (C3); underflow refuses `AggregateClosureMismatch`, overflow `Arithmetic` |
| `validate_links` generation identification (`position.generation != supply.generation`) | dropped (§4); the triple-internal generation binding and every other link check stay |
| `validate_market_init` + `NonEmptyInitialization` | unchanged; it is C0 for the founding triple plus the all-zero ledger/kernel base case |
| — (no position-init entry point existed) | new `validate_position_init` (C0): metadata, links, padding, closure bounds, kernel invariants, Active market, provably-zero triple |
| `Error::AggregateClosureMismatch` | same class, wording generalized: a presented triple exceeded, or the ledger disagreed with, the market aggregate |
| `TransitionMetadata`/`ExpectedBindings`/`StateBytes`/`TransitionOutput`, `apply`/`apply_with_evidence` signatures | **unchanged** — a second position is a second triple of accounts with its own bindings, not a wider argument list; the downstream `clutch-sbf` harness compiles and runs against this crate without modification |

Not changed here (out of this lane's scope, named for the coordinator):

- `programs/clutch-sbf/program` still carries its own inline single-position
  closure (9 accounts, no supply ledger). It is a bring-up lane for one
  instruction; porting CLO-DELTA-V1 there means adding the supply-ledger
  account to the instruction and replacing its inline equality with the C1/C2
  checks plus the C3 delta write — the same shape as this change.
- `SOLANA_REFERENCE_ADAPTER.md`'s "closed single-position model" section and
  obligation 11 should now say: the reference enforces CLO-DELTA-V1
  (this document); obligation 11's *representational* half is discharged in
  the offline model, and its runtime half (single live account per role, no
  close/reopen byte resurrection, write-lock serialization of the ledger)
  remains with obligations 1–3/9.
- `check_position_bound` in `programs/solana-layout` (the "necessary condition
  only" helper) is the layout-side statement of C2's internal half; the
  adapter now enforces both halves itself.

## 8. What this does not claim

- No Solana runtime facts: PDA uniqueness, account ownership, write locks,
  close/reopen semantics, and transaction atomicity are asserted metadata
  here, obligations there.
- No token facts: the external shadow is reference-only state; token
  conservation is obligation 6.
- No formal proof: the induction argument in §3 is stated prose, checked by
  bounded tests. If it is later formalized, the theorem is over the
  *transition relation of this crate* with the runtime obligations as
  explicit hypotheses, per obligation 14.
