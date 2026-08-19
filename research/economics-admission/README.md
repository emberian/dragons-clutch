# Economics admission lab

This directory is an exact-integer, offline falsifier for market admission,
protected-pool separation, shared-feed capitalization, and two venue fee bases.
It is independent of consensus code and does not promote a fee rate, allocation,
bounty, or token profile.

Run:

```sh
python3 -m unittest discover -s research/economics-admission -p 'test_*.py'
python3 research/economics-admission/run_lab.py
```

The model has no wallet, key, RPC, deployment, signing, network, dependency
installation, floating-point, token swap, price oracle, buyback, emission, or
future-volume path. `DREGG` is only one possible opaque `service_asset` label;
the same transitions apply to every label.

The exact equations, recommended fee-basis default, unresolved constants, and
promotion falsifiers are in
[`docs/implementation/ECONOMICS_ADMISSION.md`](../../docs/implementation/ECONOMICS_ADMISSION.md).
