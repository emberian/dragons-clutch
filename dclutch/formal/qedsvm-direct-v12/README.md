# Direct exact-ELF traces at the qedsvm v0.12 boundary

This directory records two real Agave/Mollusk executions of the exact Direct
controller ELF and the result of feeding their logical-PC traces to qedsvm
v0.12.0. It is deliberately fail-closed: **no Lean path theorem was emitted**.

The accepted trace cancels a registered intent at sequence zero and invokes the
exact Claims child ELF. The refused trace changes only `expected_sequence` to
one, returns `Custom(11)` before CPI, and preserves the complete registration
account. `evidence.json` pins every observed artifact, trace, tool, and result.

## What is checked in

- `dclutch_direct_mollusk_trace.rs`: the executable fixture. It asserts the
  accepted poststate and byte/lamport/owner/executable/rent-epoch rollback for
  the stale refusal.
- `dclutch_trace_to_pcs.rs`: converts Mollusk's traced `r11` instruction slots
  through qedsvm's own `qed_analysis::PcMap`.
- `direct-success.pcs` and `direct-stale-sequence.pcs`: the exact qedsvm lift
  inputs produced by those executions.
- `evidence.json`: machine-readable results and trust boundaries.
- `verify_capture.sh`: hashes the checked traces, exact local ELFs and `.text`,
  and the pinned qedsvm checkout; it also confirms that both lifts fail at the
  recorded unsupported two-seed PDA shape.

The raw Mollusk `.regs` files are not checked in. Their byte sizes and digests
are pinned in `evidence.json`; the checked `.pcs` files are the actual inputs
accepted by `qedlift` before it reaches the unsupported syscall model.

## Pinned reproduction

Use a local checkout of qedsvm tag `v0.12.0`, commit
`99bd5ede85374adc7fc5c835c2432ecf4e123fd1`. No fetch is required when that
checkout already exists. In its `qedsvm-rs/Cargo.toml`, add these two development
dependencies, substituting the local dClutch checkout path:

```toml
dclutch-direct-codec = { path = "/absolute/path/to/dclutch/crates/dclutch-direct-codec" }
qed-analysis = { path = "qed-analysis" }
```

Copy the two Rust sources into `qedsvm-rs/examples/`, then build the pinned Lean
and Rust tools:

```sh
lake build
cargo build --manifest-path qedsvm-rs/Cargo.toml \
  --features diff-mollusk --example dclutch_direct_mollusk_trace
cargo build --manifest-path qedsvm-rs/Cargo.toml \
  --example dclutch_trace_to_pcs
cargo build --manifest-path qedsvm-rs/Cargo.toml -p qedlift
```

Capture each path. `DCLUTCH_DIR`, `QEDSVM_DIR`, and each output directory must
be explicit absolute paths:

```sh
SBF_TRACE_DIR="$SUCCESS_TRACE_DIR" \
cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  --features diff-mollusk --example dclutch_direct_mollusk_trace -- \
  "$DCLUTCH_DIR/target/deploy" \
  "$DCLUTCH_DIR/target/deploy/dclutch_claims_proof_sbf.so" success

SBF_TRACE_DIR="$STALE_TRACE_DIR" \
cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  --features diff-mollusk --example dclutch_direct_mollusk_trace -- \
  "$DCLUTCH_DIR/target/deploy" \
  "$DCLUTCH_DIR/target/deploy/dclutch_claims_proof_sbf.so" stale
```

The controller register files are the files whose sibling `exec.sha256` equals
`e0371f3595232e8a430574fc784cd90685139265105ccadf23da5828475b4515`.
Convert them with:

```sh
cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  --example dclutch_trace_to_pcs -- \
  "$DCLUTCH_DIR/target/deploy/dclutch_controller_proof_sbf.so" \
  "$SUCCESS_CONTROLLER_REGS" direct-success.pcs

cargo run --manifest-path "$QEDSVM_DIR/qedsvm-rs/Cargo.toml" \
  --example dclutch_trace_to_pcs -- \
  "$DCLUTCH_DIR/target/deploy/dclutch_controller_proof_sbf.so" \
  "$STALE_CONTROLLER_REGS" direct-stale-sequence.pcs
```

Then run `verify_capture.sh` with the three local roots. It requires the exact
captured qedsvm binary digest and therefore validates this capture environment,
not arbitrary rebuilt host binaries:

```sh
formal/qedsvm-direct-v12/verify_capture.sh \
  "$DCLUTCH_DIR" "$QEDSVM_DIR" \
  "$HOME/.cache/solana/v1.53/platform-tools/llvm/bin/llvm-objcopy"
```

## Exact fail-closed frontier

qedsvm's Rust `ProgramImage` parses the exact ELF and both traces. `qedlift`
then refuses both at the controller-PDA validation:

```text
create_pda at pc 2806: only the single-seed (n_seeds = 1) shape is modelled so far, got 2
```

The controller correctly uses `[b"dclutch-controller-v1", bump]`. The protocol
was not weakened and qedsvm was not patched to pretend that this shape is
proved.

The independent qedsvm Lean runner has an earlier decoder boundary. It parses
the ELF header, `.text`, relocations, and collision-free function registry, but
refuses `.text` byte offset `118680` (slot `14835`, ELF address `0x1d0b8`):

```text
dc01000040000000
```

That is the current sBPF `be64 r1` instruction (`opcode 0xdc`), absent from the
v0.12 Lean decoder. This does not invalidate the Agave execution trace. It means
there is no qedsvm Lean machine execution or path theorem for these artifacts.

## Explicit unproved boundary

The runtime observations trust pinned Mollusk/Agave account serialization,
loader behavior, register tracing, PDA/SHA/memory/syscall implementations, CPI,
compute metering, and transaction rollback. The success path's Claims poststate
comes from the exact child ELF under that runtime, not from a composed Lean
theorem. The two traces are samples, not a CFG-coverage argument. Upgrade
authority and deployed ProgramData identity are outside this local artifact
capture.
