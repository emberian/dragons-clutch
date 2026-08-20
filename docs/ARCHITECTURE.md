# Dragon's Clutch V1 architecture

Status: planning scaffold; no deployed program, client, market, or financial
authority.

## 1. Architectural thesis

Dragon's Clutch is a compiler and settlement protocol for finite objective state
spaces. It is not a generic prediction-market factory, generic exchange, or
general-purpose computer. V1 admits only a closed collection of semantic objects
whose bounds, encoding, transitions, and failure behavior are known in advance.

The architecture has two deliberately different planes:

```text
semantic plane                         hostile integration plane

partition and payoff algebra          Solana accounts and instruction bytes
complete-set conservation       <->    Token-2022 transfers, minting, burning
shared summary transitions             authenticated source account parsing
specialized batch relation             transaction/CPI construction
protected-pool accounting              static-client projections
```

Eggcrate owns the semantic plane. Adapters translate hostile external state into
validated Eggcrate inputs and translate successful transitions into narrow CPI
intents. An adapter never invents an economic amount, outcome, score, fee, or
payout.

## 2. Dependency direction

```text
                 Lean model — substrate of record
                           |
                    canonical vectors
                           |
closed compiler -->     Eggcrate      <-- specialized BatchRelationV1
                           |
                   fixed wire contracts
                           |
       +-------------------+-------------------+
       |                   |                   |
Solana adapter       source adapters      static Glass
       |                   |                   |
 Token-2022        authenticated state      wallet/RPC
```

Dependencies point down toward the kernel, never from the kernel toward Solana,
Token-2022, a source SDK, a wallet, or the client. The hand-written proof models
consume the same semantic vectors but do not share executable implementation.

Lean is the proof substrate of record
([adr/0005-lean-proof-substrate-of-record.md](adr/0005-lean-proof-substrate-of-record.md),
adopted 2026-08-20 —
[decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) item 2,
superseding ADR-0003's Verus-first arrangement). Verus is retained only for
checked-Rust-subset results verifying actual executable bodies under
digest-pinned contracts — today the transfer-arithmetic refinement plus the
digest-pinned scalar batch shadow with its named excluded sources; the Rocq
shadow role is retired. The model's correspondence to the Rust remains manual
and disclosed: no model theorem is evidence about the Rust, the ELF, or a
deployment.

## 3. Semantic ownership

Every persisted fact has exactly one owner. Projections may cache it, but cannot
create a second authority.

| Fact | Semantic owner | Untrusted projections |
|---|---|---|
| Collateral profile | immutable `Realm` | client labels, index records |
| State-space semantics | content-addressed `Template` | prose and charts |
| Liability and lifecycle | market-local `Instance`/`Market` | indexer status |
| Claimant collateral | market-local `Hoard` | wallet balances, UI totals |
| Internal balances | Eggcrate-owned `Position` | client portfolio view |
| Materialized supply accounting | `SupplyLedger` plus authenticated mint supply | token lists |
| Source semantics | immutable `FeedSpec` | provider descriptions |
| Accepted path progress | `FeedHead` and archive summaries | keeper dashboards |
| Window fact | immutable `WindowResult` | cached charts |
| Frozen order set | `EpochBook` page closure | solver book image |
| Clearing result | finalized `BatchRelationV1` witness | candidate preview |
| Resolution | immutable `Hatch` | human outcome label |
| Release identity | checked release manifest | GitHub Pages or DNS pointer |

The same byte cannot simultaneously represent two ownership phases. In
particular, an asset cannot be both an order reservation and a final settlement
pot, and a materialized Egg cannot remain credited internally.

## 4. Kernel domains

Eggcrate remains `no_std`, `no_alloc`, fixed-layout, safe Rust, and total. Its
public domains are deliberately small:

1. fixed-width exact arithmetic and one named rounding boundary;
2. canonical closed-enum partition compilation and unique cell selection;
3. bounded payoff vectors and finite payout-vector sets;
4. complete-set split, merge, materialize, dematerialize, and redemption;
5. protected-pool and liveness-booking transitions;
6. associative observation summaries and Window closure;
7. the specialized batch witness verifier and score relation;
8. canonical codecs for every signed or persisted semantic object.

The kernel does not parse JSON, query an RPC, derive a PDA by search, call a token
program, read a clock, hash a human label, or decide whether an external source is
legally or economically suitable.

## 5. Instance lifecycle

```text
Draft
  -> Capitalized
  -> Active
  -> ObservationClosed
  -> Resolved(Hatch)
  -> Redeeming
  -> Retired

Active
  -> Degraded
  -> Repaired -> Active/ObservationClosed
  -> FailureClosed -> Resolved(Hatch)
```

Every edge has a closed input form and explicit refusal class. No identity gets a
discretionary transition. A failure edge selects only from the finite payout set
frozen at activation; the preferred failure policy remains an open E0 decision.

## 6. Claim representation

The native representation is a fixed internal Position. The interoperability
boundary is a canonical Token-2022 mint per outcome.

```text
internal Egg_i --materialize--> Token-2022 Egg_i
internal Egg_i <--dematerialize-- Token-2022 Egg_i
```

For each outcome `i`:

```text
total_i = internal_supply_i + accounted_external_supply_i
```

Split increases every `total_i` and Hoard collateral by the same quantity. Merge
decreases all of them equally. Materialization and dematerialization preserve
`total_i`. A direct external burn is a donation and may only lower the
conservative external ledger after authenticated reconciliation.

This boundary keeps native batches compact while preserving an ordinary,
one-outcome-at-a-time Token-2022 escape hatch. V1 does not mint portfolio tokens
or force users to create all outcome token accounts.

## 7. Shared path substrate

Markets do not each replay the same price path. A `FeedSpec` selects one frozen
source adapter and time grid. `FeedHead` advances monotonically through bounded
interval summaries. Archive pages preserve the summaries required by every live
window, and immutable `WindowResult` accounts may be shared by all Instances with
the same semantic identity.

Only explicitly registered associative summaries are admissible. A statistic
that cannot be recovered from them is refused; the protocol does not silently
approximate it from insufficient history. See [ACCUMULATOR_PLAN.md](ACCUMULATOR_PLAN.md).

## 8. Specialized batch relation

The native venue does not execute a generic matching VM. It checks one bounded
relation over a frozen order set:

```text
BatchRelationV1(public_epoch, candidate_witness) -> accept | error
```

The relation normalizes orders, checks a simplex price vector, checks eligibility
and exact portfolio limits, accounts for virtual complete-set conversion, folds
per-page asset deltas, applies one frozen pro-rata/remainder rule, recomputes the
public score, and proves closure of collateral and every Egg.

Search remains permissionless and offchain. Authority remains in the exact
onchain relation. V1 is transparent: orders and accepted witnesses are public.
Confidential execution is neither implemented nor claimed here. See
[SPECIALIZED_BATCH_RELATION.md](SPECIALIZED_BATCH_RELATION.md).

## 9. Static-client boundary

Glass is a reproducible transaction-construction and inspection tool. It can:

- compile a known Template and exact payoff vector;
- validate onchain state and a release manifest;
- solve small books or construct a candidate witness;
- display exact units, bounds, fees, failure policy, and postconditions;
- expose permissionless paid work for explicit user approval.

It cannot define truth, select a result, keep a required secret, run an
authoritative index, or submit anything without the user's wallet. GitHub Pages
is a mutable mirror; an immutable content digest and manifest are the release
identity.

## 10. Named trust boundaries

The intended claim chain is:

1. Lean proves a named abstract theorem over a named model digest (the
   substrate of record, ADR-0005).
2. Verus proves a named property of the exact Eggcrate source digest — its
   retained checked-Rust-subset role, and the only link in this chain that
   reaches an executable body.
3. cross-runtime vectors compare that source under host and pinned SBF builds.
4. adversarial tests exercise the unverified Solana adapter.
5. a reproducible manifest binds source, proof artifacts, schemas, and ELF.
6. a deployment manifest, if separately authorized, binds that ELF to program
   data and names upgrade authority and fee recipients.

No link proves the next one automatically. Solana, Token-2022, source programs,
the SBF compiler, and the client remain explicitly named assumptions or adapters.

## 11. Hard stop boundaries

Architecture work may produce source, proofs, fixtures, local-validator tests,
and a static unsigned client. It grants no authority to:

- sign or submit a transaction;
- access a wallet or secret;
- deploy to devnet or mainnet;
- create a market or accept funds;
- solicit users, trade, make markets, or receive venue revenue;
- describe a deployment as official; or
- treat legal research, a meeting, or a passing test suite as closure of Gate L0.

The mainnet boundary is specified in
[DEPLOYMENT_REVENUE_BOUNDARY.md](DEPLOYMENT_REVENUE_BOUNDARY.md) and
[ENGINEERING_PLAN.md](ENGINEERING_PLAN.md).
