# Dragon cost laboratory

This directory is a deterministic, standard-library-only lower-bound and byte-layout laboratory.
It performs no RPC call, simulation, signing, submission, deployment, account creation, package
installation, or wallet access.

Run it with the repository's Python 3:

```text
python3 benchmarks/cost_lab.py check
python3 benchmarks/cost_lab.py summary
python3 benchmarks/cost_lab.py abi-audit
python3 -m unittest discover -s benchmarks/tests -v
```

`abi-audit` re-derives every `account_len` constant from `programs/solana-layout/src/lib.rs` on
disk and refuses if the landed arm below has gone stale. It reads one source file, is not part of
golden closure, and is the tripwire that keeps a cost conclusion from being attributed to a layout
the codec no longer has.

To regenerate the checked-in deterministic artifacts after an intentional model change:

```text
python3 benchmarks/cost_lab.py generate --output benchmarks/golden
python3 benchmarks/cost_lab.py check
```

## Arms

Every row carries an `arm`.

- `layout_hypothesis` is the original design sketch: 193 rows, retained unchanged so its
  falsifications stay readable. It is a design arm, not a description of the current program.
- `abi_landed` reads the landed codec in `programs/solana-layout` (pinned at commit `efb0ed5`)
  and the landed relation bounds in `crates/clutch-batch`: the 15-account family and its exact
  widths, the forced 1,819-byte/16-record order page, the 64-order epoch book, and the nine intent
  payload widths. `constants.json` stores each width as the codec's own field terms, and the
  harness refuses to run unless every pinned total equals the sum of its terms.
- `abi_differential` carries the exact integer delta for every object present in both arms, plus
  the objects that exist in only one of them.

A landed width is an encoding fact, never a measured cost. The landed arm reports no compute
units at all, and the harness refuses any landed or differential output key ending in `_cu`.

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
- `landed_codec_constant` means the value is the landed codec's own `account_len`/`encoded_len`
  expression, re-derived from its field terms rather than quoted.
- `layout_hypothesis_not_landed` marks the parts of a landed row that are still guesses: which
  accounts an instruction locks, and whether one intent is one Solana instruction.
- `absent_no_landed_verification_instruction` means the landed ABI has no instruction for this
  step, so the arm reports work and rent and emits no wire byte count rather than inventing one.

`constants.json` pins all external values and source revisions. `golden/matrix.json` contains the
full rows, `golden/matrix.csv` is a compact review surface, `golden/SUMMARY.md` is derived, and
`golden/checksums.sha256` closes the three generated artifacts.
