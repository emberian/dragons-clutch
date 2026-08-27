# Provisional source snapshots

This directory holds machine-readable observations used to design and test a
source adapter. They are evidence records, not compiled registry entries,
release identities, deployment manifests, or authority to transact.

The 2026-08-22 devnet snapshot is checked entirely offline against its dated
review and local-clone script:

```sh
python3 programs/clutch-sbf/source-profiles/check_provisional_snapshot.py
```

The checker parses local files only. It validates the schema and encodings,
requires the clone script and review to name the same cluster identities, checks
the Anchor discriminator independently, and refuses any status that could be
read as promoted. It does not contact RPC, read a wallet, start a validator, or
alter the compiled source registry.

- Record: [`devnet-real-source-snapshot-2026-08-22.json`](devnet-real-source-snapshot-2026-08-22.json)
- Human review: [`docs/reviews/DEVNET_REAL_SOURCE_SNAPSHOT_2026-08-22.md`](../../../docs/reviews/DEVNET_REAL_SOURCE_SNAPSHOT_2026-08-22.md)
- Read-only clone helper: [`scripts/run_pyth_devnet_clone.sh`](../scripts/run_pyth_devnet_clone.sh)

Promotion requires a new, explicitly authorized capture and its own release
review. Editing this record cannot create a release.
