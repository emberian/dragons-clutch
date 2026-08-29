# Resumable devnet flight

`devnet_flight.py` is a thin, local process coordinator for the existing
Decision-0012, market, activity, reconciliation, site, and Pages commands. It
does not read keys, perform RPC, construct transactions, calculate extension
sizes, or interpret any protocol evidence. Those remain owned by the supplied
commands.

The flight file is strict JSON with an exact devnet target and one argv array
per phase. Arrays are passed directly to `subprocess.run(..., shell=False)`.
The required IDs, in order, are:

```text
candidate
buffer:custody buffer:resolution buffer:claims buffer:trading buffer:core
upgrade:custody upgrade:resolution upgrade:claims upgrade:trading upgrade:core
sponsored-market-open participant-lifecycle direct-lifecycle terminal-lifecycle
finite-activity reconcile site-refresh wrapper-pages-checkpoint
```

Optional `extend:custody`, `extend:resolution`, `extend:claims`,
`extend:trading`, or `extend:core` entries belong after `candidate` and before
the buffer stage. Include one only when the existing checked baseline and
candidate command determine the ProgramData needs `devnet-upgrade-extend-v1`.
The driver neither repeats that arithmetic nor decides whether an extension is
needed. Each buffer argv must use the existing
`--stop-after-buffer-ready` boundary; its paired Upgrade argv resumes without
that flag. The five Upgrade entries are canonical Custody-to-Core order.

Plan mode is entirely local and key-free; it parses the flight and prints only
argv SHA-256 values:

```sh
python3 tools/release/devnet-flight/devnet_flight.py \
  --flight /absolute/flight.json --journal /absolute/flight-journal.json
```

Execute mode dispatches only argv arrays already carrying the existing exact
`--i-mean-devnet EtWTR...` acknowledgement and a child command's existing
mutation phrase (for example `--execute`, `--i-accept-*`, or
`--i-kept-*`). It journals `before-external-mutation` and fsyncs it before
every mutating child. The journal contains only public phase names, state, and
argv digests; child output is not copied into it. A failed child leaves that
phase failed, while a finalized phase is skipped on a rerun.

```sh
python3 tools/release/devnet-flight/devnet_flight.py \
  --flight /absolute/flight.json --journal /absolute/flight-journal.json --execute
```

The caller provides the real commands, including checked-release descriptors
and build refs, `devnet-upgrade-extend-v1`/`devnet-upgrade-v1`, the accepted
sponsored-Pyth market command, Activity-v3, `devnet-reconcile`, and the
repository's site/Pages wrapper. Do not put key bytes in the JSON; key paths
are consumed only by the existing child command in execute mode.

Run the isolated coordinator tests with:

```sh
python3 -m unittest -v tools/release/devnet-flight/test_devnet_flight.py
```
