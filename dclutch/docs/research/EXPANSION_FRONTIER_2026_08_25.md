# Expansion frontier after the omission review — 2026-08-25

Status: active product/theory and implementation direction. This document
reopens conservative prototype choices without weakening the named invariants.
It is not release, deployment, or formal-verification evidence.

## Principle

The successor preserves invariants, not accidental mechanisms.

Hard boundaries are narrow:

- one live writer and one persisted truth for each economic fact;
- no hidden rounding, implicit unit conversion, or unclassified value;
- no Hoard or anticipated-revenue funding of liveness;
- no caller, client, mock, or adjacency fact becoming authority merely by
  inclusion;
- no claim of solvency, optimality, equivalence, or deployment beyond its
  exact certificate and evidence boundary.

Thin Core, five execution roles, interpretation, categorical claims, exact
denominator materialization, covered quote bins, strict Token-2022 layouts,
and permanent RentCredit are current safe profiles. They are not all final
ontology.

## Accepted immediate corrections

### One extensible Trading root

Each Trading capability owns one account:

```text
CapabilityRootHeaderV1 (232 immutable bytes)
|| descriptor-selected mutable root-state tail
```

The fixed header binds release set, Market, generation, and exact manifest
selection. `CapabilityProgramV1.root_schema_id` identifies the tail schema;
the descriptor states its exact byte width. Family semantics mutate only the
tail and must preserve the header byte-for-byte. This avoids a generic root
plus a redundant family root, one extra PDA, and one extra rent obligation.

### Semantics are independent of execution strategy

`CapabilityProgramV1` owns transition and effect meaning. V1 Trading executes
it through TransitionVM. A future accepted strategy may instead use a
Registry-authenticated stateless accelerator, provided Trading remains the
sole state/effect commit authority.

## Frontier 1: certified execution strategies

A future `ExecutionStrategyCertificateV1` should bind at least:

```text
capability_program_id
account_profile_id
effect_schema_id
strategy_kind = interpreted | stateless_aot
accelerator program and ProgramData
artifact release
compiler and toolchain digests
translation-validation or theorem digest
```

The accelerator consumes the authenticated register projection and returns
the same canonical effect plan. It cannot own Trading roots, Claims, Custody,
or Resolution state and cannot perform unapproved CPI.

The first comparison is compiled Direct against the same interpreted Direct
descriptor. Acceptance and refusal equivalence, return-data producer, stale
artifact refusal, late rollback, ELF rent, packet bytes, and CU must all be
measured. Family/AOT measurement artifacts therefore remain non-authoritative
evidence inputs until this comparison finishes; they are not deleted merely
because V1 interpretation works.

A variable set of state-owning roles is separate research. It requires an
actual capability whose exclusive syscall or canonical-state boundary cannot
fit behind Trading.

## Frontier 2: certified nonnegative liability bases

Categorical claims become the optimized member of a broader exact basis:

```text
LiabilityBasisV2 {
    basis_width: K,
    payout_scale: Q,
    evaluator_release,
    certificate_schema,
    capacity_profile,
}
```

For every admitted terminal result `x`, the evaluator returns integer payouts
in Realm collateral atoms:

```text
p_i(x) >= 0
sum_i p_i(x) = Q
```

One complete-set lot locks `Q` collateral atoms and creates one lot of every
basis claim. For outstanding supplies `T_i`, exact terminal liability is:

```text
L_x(T) = sum_i T_i * p_i(x)
H >= max_x L_x(T)
```

Categorical is exactly `Q = 1` with a one-hot payout vector. The first richer
slice is a two-claim capped ramp and exact complement. It must prove evaluator
totality, nonnegativity, exact partition sum, arithmetic bounds, and split,
merge, trade, resolution, and redemption preservation. The rational-to-integer
apportionment boundary is named once; an approximation is never described as
an exact continuous payoff.

Existing Product Payoff V2 exact-rational evaluation and wide checked
arithmetic are starting material. Market and Claims layouts do not change
until the pure theorem and hostile translation corpus are accepted.

## Frontier 3: exact denominated claim shards

Exact-denominator Structured remains the zero-remainder fast profile. It is
not the only admissible fractional representation.

For outcome `i`, denominator `D`, native claims locked in the canonical shard
custody Position `C_i`, and Token-owned shard Mint supply `F_i`:

```text
F_i = D * C_i
```

Exact actions are:

- denominate `q` native claims and mint `q*D` shard atoms;
- burn exactly `q*D` shard atoms and reconstitute `q` native claims;
- after resolution, burn `D` winning shards per collateral atom;
- burn losing shards for zero; and
- keep fewer than `D` winning shards transferable and aggregable rather than
  rounding or creating a hidden per-holder credit.

Structured V2 backs each receipt atom with exact shard atoms. For normalized
coefficient `c_i/D`, receipt supply `S`, and Structured shard custody `K_i`:

```text
K_i = S * c_i
```

The resulting representation graph is deliberately finite:

```text
Structured receipt -> exact claim shard -> native Position -> Market liability
```

The required evidence covers denomination, reconstitution, terminal payout,
no double redemption, denominator overflow, ordinary Token-2022 transfer and
aggregation, zero-supply retirement, Custody payout, and byte-exact rollback
after late refusal.

## Frontier 4: scenario-solvent Dealer capital

Dealer V1's gross covered quote-bin ladder remains a strong simple profile.
It is not the capital-efficiency ceiling.

A generalized Dealer admits exact terminal-scenario coverage:

```text
equity(s) = collateral
          + winning_claim_inventory(s)
          - obligations(s)

min_s equity(s) >= locked_capital_floor
```

Canonical Claims Positions remain the inventory owner. Dealer may derive a
same-transaction plan that splits the minimum complete sets needed for a
delivery and merges common post-trade inventory; it does not persist a shadow
inventory. Anticipated fees are never capital.

Pricing may evolve through deterministic descriptor-selected policy,
consent-bound capital tranches, or quiescent epoch transitions. Unilateral
repricing of incumbent capital remains invalid. True defaulting credit is a
separate open problem requiring an explicit default instrument and loss model.

## Frontier 5: compositional representation DAGs

Reject recursive authority, not composition. Product tooling may admit a
content-addressed acyclic recipe expression and prove canonical flattening to
native exposure. Runtime starts with the typed depth-two
Structured-to-Shard-to-Native graph. Each node has one supply owner, one
backing edge, and a decreasing rank. Arbitrary wrapper-on-wrapper live custody
does not enter the profile until cycle refusal, terminal payout, retirement,
and rent closure are proved.

## Frontier 6: Token behavior profiles

The current zero-decimal PermissionedBurn Mint and base Token Account are the
strict default, not the only forever-supported representation. A Token behavior
profile binds the sorted Mint/Account extension set, authorities, permitted
instructions, parser release, and effect release.

The first lift is display decimals and inert metadata. Transfer fees, hooks,
confidential state, pausing, or permanent delegates each require a separate
conservation, authority, liveness, CPI, and rollback argument. No profile may
introduce shadow supply or silent reconciliation.

## Frontier 7: lifecycle-scoped refund sinks

RentCredit V1 remains valid for existing objects, but permanence is not an
economic invariant. A successor refund sink binds one immutable beneficiary,
Market generation, and authenticated producer subtree. It may close to that
beneficiary only after Market terminal state and exact zero outstanding
producers. It has no caller-selected redirect or partial authority migration.

The implementation slice must first inventory every possible late rent or
donation producer and prove that the terminal counter/certificate is complete.

## Frontier 8: measured width lifting

Remove remaining `N = 2..16` dispatch before introducing paging. Measure the
largest contiguous runtime-width profile with borrowed authenticated vectors.
Where chain limits require paging, a new Product or manifest profile commits
canonical page order and one aggregate identity, proves completeness and
acyclicity, and uses staged computation certificates. Economic commit remains
bounded and atomic; pages cannot make a partially verified liability state
visible.

## Implementation order

1. Finish the common Trading header/tail and interpreted V1 vertical.
2. Compare Direct interpretation with a stateless AOT certificate.
3. Prove and execute the two-claim ramp/complement basis in the pure kernel.
4. Implement exact claim denomination before expanding Structured.
5. Add Dealer scenario reserve/netting as a pure no-allocation kernel before
   changing custody or LP shares.
6. Lift Token, refund, and width profiles one measured slice at a time.

This work expands the protocol while retaining a runnable conservative profile.
No current safe path is called final merely because it landed first.
