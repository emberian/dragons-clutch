# Economics differential fixtures

Status: **PROPOSED model vectors.** Nothing in this directory is a promoted
protocol constant, a frozen policy, or evidence for `P-SOLV-01` / `P-FEE-01` on
its own. The vectors exist so that two independent implementations can be shown
to classify and settle identical inputs identically.

Source of the semantics: `docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md`
sections 1, 2 and 3.4. Producer: `research/economics/fixtures.py`.

## The cross-language contract

These files are **language-neutral vectors that every consumer must pass**. The
Python economics lab is one consumer (`research/economics/test_fixtures.py`); a
future Rust consumer in `crates/clutch-kernel` and `research/vertical-model` is
the other. The contract is:

1. **Exact integers only.** No floats, no rationals, no scaled decimals. Every
   quantity, weight, denominator, price, fee numerator, carry and payout is an
   exact integer in the file.
2. **A fixture that fails on either side is a finding, not a fixture to edit.**
   Minimized failures become permanent named vectors here; vectors are never
   deleted or relaxed to make an implementation pass.
3. **Every vector names its policy arm.** `kernel_baseline` is the landed
   `crates/clutch-kernel` behaviour. `one_hot`, `lots` and `credit` are the
   PROPOSED candidates (a1), (b1) and (c) of the policy analysis. A consumer
   that implements only the landed arm reports the others as unsupported; it
   must still match `kernel_baseline` exactly.
4. **Shared refusal vocabulary.** Refusals are compared by `error_class`
   strings, not by native error types. The vocabulary is listed in each file's
   `error_classes` field; the first fifteen map one-for-one onto
   `clutch_kernel::Error`, while `lot_violation` and `no_credit` belong to the
   proposed candidate arms and have no kernel counterpart today.
5. **Deterministic bytes.** UTF-8, two-space indent, sorted keys, trailing
   newline, no timestamps, no host paths, no randomness, no environment
   capture. Regenerating must reproduce the committed bytes exactly:

   ```sh
   python3 research/economics/fixtures.py
   git diff --exit-code fixtures/economics
   ```

## Files

### `admission_vectors.json` (`EXP-ALIGN-01`)

A payout set (`outcomes`, `count`, integer `weights`, `denominator`) and, per
policy arm, `admit` or `refuse` with an `error_class`. Covers weight sums below
and above the denominator, a weight above the denominator, mixed denominators, a
zero denominator, nonzero weight padding, nonzero vector padding, outcome- and
payout-count bounds, and the one-hot versus fractional split. Admitted sets also
carry `derived_lots` (`L_i = D / gcd(D, {v_i != 0})` and
`L_split = lcm_i L_i`), which are pure functions of the set and require zero
stored state.

### `trace_vectors.json` (`EXP-ALIGN-02`)

Market terms plus one shared operation list, replayed independently under every
policy arm, with per-step `ok(payout)` / `refuse(error_class)` and the final
`collateral` / `total_supply` / `credit_total` / per-position balances.

Trace vector #1 is the P1-A fixture of the adversarial review: weights `[1, 1]`
over `D = 2`, split one atom, resolve. Under `kernel_baseline` both per-outcome
redemptions refuse with `remainder_required` forever while the market stays
solvent; the proposed complete-set redemption clears it; `one_hot` refuses the
market at admission; `lots` refuses the split; `credit` pays a floor and accrues
an exact `1/D` credit.

Refused steps never change state, so a consumer may replay the whole list and
compare step by step. Each step carries a `transition_class`:

- `landed_kernel` -- exists in `crates/clutch-kernel` today, including the
  internal terminal complete-set redemption landed in commit `d60ccf3`;
- `external_adapter` -- a bearer Token-2022 transfer, deliberately ungated;
- `lab_extension` -- complete-set redemption presented from the *external* side,
  which the kernel does not offer; it appears only to show what materialization
  strands;
- `proposed_candidate_c` -- the credit claim, which exists in no implementation.

A consumer that implements only landed transitions replays the prefix up to the
first non-landed step that is expected to succeed.

### `fee_vectors.json` (`EXP-ALIGN-03`)

Three kinds:

- `single_egg_schedule` -- fills, carry domain, close policy and fee-side arm to
  per-leg `paid`/`carry`, terminal charges, fee pot, payer debits and credits,
  the allocation triple, and the required-true conservation flags
  (`payer_identity`: `sum(buyer debits) - sum(seller credits) = fee pot delta`;
  `hoard_untouched`: no fee leg ever debits the Hoard);
- `dispersion_point` -- payoffs and prices to `G_num`, `fee_num`, denominator,
  floor, carry and terminal ceiling;
- `allocation_point` -- a collected pot to the maker/executor/treasury triple
  under several executor caps.

Every vector carries a `derivation` string stating the arithmetic in full, so a
reviewer can check the expectation without running either implementation.

`kappa = 4/1000`, the 60/15/25 allocation, the price scales and the executor
caps in these files are experimental arms. Passing these vectors is evidence
that two implementations agree, and nothing more.
