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

The models are independent falsifiers, not consensus code, proof artifacts,
network measurements, or promoted protocol constants. See
[`docs/implementation/ECONOMICS_LAB.md`](../../docs/implementation/ECONOMICS_LAB.md)
for hypotheses, interpretations, and stop conditions.

