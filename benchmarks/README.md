# Dragon cost laboratory

This directory is a deterministic, standard-library-only lower-bound and byte-layout laboratory.
It performs no RPC call, simulation, signing, submission, deployment, account creation, package
installation, or wallet access.

Run it with the repository's Python 3:

```text
python3 benchmarks/cost_lab.py check
python3 benchmarks/cost_lab.py summary
python3 -m unittest discover -s benchmarks/tests -v
```

To regenerate the checked-in deterministic artifacts after an intentional model change:

```text
python3 benchmarks/cost_lab.py generate --output benchmarks/golden
python3 benchmarks/cost_lab.py check
```

## Evidence classes

- `measured_local_serialization` means the harness emitted concrete synthetic Solana transaction
  bytes and counted them. Placeholder signatures, addresses and blockhashes are deterministic and
  have the real encoded widths. This is a wire-format measurement, not an executable transaction.
- `analytical_lower_bound` means a count follows from the named operation topology or information
  bound: accounts, CPIs, instruction trace entries, order authentication, asset closure, or dot
  products. It is not a validator measurement.
- `analytical_package_default_not_cluster_measurement` means refundable rent principal is computed
  from the pinned `solana-rent` package default. A target cluster's Rent sysvar can differ and must
  be bound before funding.
- `layout_hypothesis` means Dragon has not frozen this account/record/instruction ABI. These rows
  are meant to falsify layouts early, not bless them.

`constants.json` pins all external values and source revisions. `golden/matrix.json` contains the
full rows, `golden/matrix.csv` is a compact review surface, `golden/SUMMARY.md` is derived, and
`golden/checksums.sha256` closes the three generated artifacts.
