# Economics laboratory

This directory contains deterministic, host-only cryptoeconomic reference models.
It uses Python's standard library, integer atoms, and exact rational arithmetic.
It has no RPC, wallet, signing, deployment, trading, or dependency-install path.

Run the property suite:

```sh
python3 -m unittest discover -s research/economics -p 'test_*.py'
```

Run the deterministic scenario report:

```sh
python3 research/economics/run_lab.py
```

Regenerate the language-neutral differential fixtures (byte-identical output;
a diff means an expectation moved and needs review, not a rewrite):

```sh
python3 research/economics/fixtures.py
git diff --exit-code fixtures/economics
```

Modules:

- `model.py` -- exact reference transitions and accounting equations, including
  the integer kernel mirror (`WeightedBook`, payout candidate arms, redemption
  lots) and the payer-debit fee accounting;
- `experiments.py` -- the executed rows of the
  [`POLICY_ANALYSIS_LOTS_FEES.md`](../../docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md)
  section 5 falsifier matrix, each carrying its own falsification condition;
- `fixtures.py` -- hand-authored differential vectors written to
  [`fixtures/economics/`](../../fixtures/economics/);
- `test_lab.py`, `test_alignment.py`, `test_fee_policy.py`, `test_fixtures.py`
  -- the property suite;
- `run_lab.py` -- stable sorted JSON scenario report.

The models are independent falsifiers, not consensus code, proof artifacts,
network measurements, or promoted protocol constants. See
[`docs/implementation/ECONOMICS_LAB.md`](../../docs/implementation/ECONOMICS_LAB.md)
for hypotheses, interpretations, and stop conditions.

