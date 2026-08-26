# Token-2022 V11 ProgramTest fixture

`token-2022-v11.provenance` is the authority for the real Token-2022 ELF used
by the Rational Representation and Rational Lifecycle ProgramTests. The source
is the crates.io `spl-token-2022` 11.0.0 archive, including its published
`Cargo.lock`, and the build uses cargo-build-sbf 4.0.0 with platform-tools
v1.53 and SBF rustc 1.89.0.

The canonical artifact is host-bound. Rebuilding the exact pinned archive and
lockfile on Linux x86_64 produces `e2acdf…f5697`; two clean extraction-path
builds on macOS arm64 both produce `447ca3…c25d`. Both files are 615704 bytes,
but their byte digests differ. The earlier ProgramTest script recorded only the
tool version label, so it could not explain or reproduce this cross-host
difference.

The accepted fixture remains the Linux x86_64 artifact. On that host, run the
checked script with `TOKEN_2022_V11_CRATE` pointing to the pinned archive. On a
different host, first build the artifact on the canonical host using the
manifest's exact command, then provide its local path through
`TOKEN_2022_V11_ELF`. The script authenticates the archive, embedded lockfile,
upstream revision/path, cargo-build-sbf archive, platform-tools manifest,
toolchain label, and final ELF digest. It never accepts the macOS audit digest
as a substitute. Both ProgramTest launchers call the same checked fixture
builder; neither test carries a separate accepted digest.
