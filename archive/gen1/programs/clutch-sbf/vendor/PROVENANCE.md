# Vendored third-party source

## `solana-define-syscall 5.1.0`

- Upstream: <https://crates.io/crates/solana-define-syscall> (Anza, `anza-xyz/solana-sdk`).
- License: Apache-2.0. `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock` and
  `.cargo_vcs_info.json` are the unmodified crates.io package contents; nothing
  in the tree has been edited.
- crates.io `.crate` sha256, taken from the local sparse index at
  `~/.cargo/registry/index/index.crates.io-1949cf8c6b5b557f/.cache/so/la/solana-define-syscall`:
  `21e14a4f604117f379840956a8fc8695e4c84f5b0ebed192f31f60d9b85d581d`.
- Copied verbatim from
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/solana-define-syscall-5.1.0`,
  minus the cargo-internal `.cargo-ok` marker.

### Why it is here

`solana-address 2.6.1` requires `solana-define-syscall ^5.1.0`.  This host has
the 5.1.0 *source* unpacked in the Cargo cache but not the corresponding
`.crate` archive, and `cargo-build-sbf` runs `cargo metadata`, which insists on
an archive for every package in the resolve graph on every platform.  With no
network and no panamax mirror on this host there is no way to fetch it, so the
workspace patches this one dependency to a path.

This is a build-plumbing workaround for an offline host, not a fork.  Deleting
the directory and the `[patch.crates-io]` entry in
`programs/clutch-sbf/Cargo.toml` restores an ordinary registry dependency the
moment the archive is available.  See `docs/implementation/SBF_BRINGUP.md`.
