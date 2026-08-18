# E0 toolchain compatibility lab

This directory contains an offline, non-deploying probe for the shared
Verus/upstream-Rust/Anza-SBF source boundary. It is intentionally independent
of the not-yet-created workspace and has no Solana SDK, RPC, wallet, key,
program ID, CPI, or deployment path.

Run from the repository root:

```sh
CARGO_NET_OFFLINE=true toolchain/scripts/run_lab.sh
```

The script builds the exact `no_std_core` source with the pinned upstream host
toolchain and `cargo-build-sbf`, executes the host assertions, rebuilds SBF a
second time, compares the two SBF artifact hashes, and scans the source for
prohibited proof shortcuts. Temporary outputs are left in `/tmp` for inspection
and are not repository evidence until captured with their source and toolchain
manifest.

`run_verus.sh` is a separate explicit probe. It exits with status 2 when Verus
is not installed; it never installs a tool or contacts the network.

The observed pin snapshot is in [`versions.env`](versions.env). The complete
interpretation, current blockers, and promotion gates are in
[`docs/implementation/TOOLCHAIN_SPIKE.md`](../docs/implementation/TOOLCHAIN_SPIKE.md).
