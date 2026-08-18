# E0 toolchain compatibility spike

Status: **partial probe pass; E1 promotion blocked**

This is an offline compatibility experiment, not a deployment or a formal
verification result. It uses a deliberately tiny source file and synthetic
integer operations. It does not access RPC, keys, wallets, a validator, an
account, a program ID, or a Solana cluster.

## Question and boundary

Can one small, safe, `no_std`, `no_alloc` Rust source be compiled unchanged by
the pinned upstream host toolchain and Anza SBF toolchain while retaining total
executable checks? This spike does not yet answer whether Verus verifies that
source, whether an adapter produces a valid deployable ELF, or whether Solana
runtime behavior agrees.

The source under test is [`toolchain/probes/no_std_core/src/lib.rs`](../../toolchain/probes/no_std_core/src/lib.rs). It contains checked `u128`
fee arithmetic, a closed-open interval classifier, and a checked debit. It has
no unsafe code, allocation, floating point, FFI, target-specific executable
branch, or external dependency. The host assertions are in
[`toolchain/probes/host_harness/src/main.rs`](../../toolchain/probes/host_harness/src/main.rs).

## Reproduction

The single entrypoint is:

```sh
CARGO_NET_OFFLINE=true toolchain/scripts/run_lab.sh
```

It invokes `rustup run` with the pinned host toolchain, `cargo --offline` for
the host harness, and `cargo-build-sbf` with `CARGO_NET_OFFLINE=true`. It builds
twice into fresh temporary target directories. The exact Verus command is
separate:

```sh
toolchain/scripts/run_verus.sh
```

That command is intentionally blocked until a reviewed Verus release is
installed and pinned. It must be run against the same source digest; no copied
or preprocessed source is accepted.

## Observed pin snapshot

Observed on 2026-08-17, `aarch64-apple-darwin`:

| Component | Pin | Observation |
| --- | --- | --- |
| host Rust | `1.89.0` / commit `29483883e` / LLVM `20.1.7` | installed and used |
| Anza/Solana CLI | `solana-cli 4.0.2` | installed |
| SBF builder | `cargo-build-sbf 4.0.0` | installed and used |
| SBF platform tools | `v1.53` | reported by installed SBF toolchain |
| SBF embedded Rust | `rustc 1.89.0` | reported by installed SBF builder |
| Z3 | `4.16.0` | installed; not invoked by this no-proof probe |
| Verus | **UNAVAILABLE** | no `verus` or `vargo` binary found |

The machine-readable snapshot is [`toolchain/versions.env`](../../toolchain/versions.env).
The snapshot is not a substitute for a future lock containing an exact Verus
release, source revision, verifier configuration, `vstd` revision, and solver
configuration.

## Results

The run on the snapshot produced:

```text
source_sha256=10b2087683d3c2cb423768eb9c612c00ea929b171835c15d3d16792d6b8b19ac
host_rustc=rustc 1.89.0 (29483883e 2025-08-04)
sbf_build=cargo-build-sbf 4.0.0
host_build=PASS
sbf_build=PASS
sbf_reproducibility=PASS
prohibited_source_scan=PASS
verus=UNAVAILABLE
verus_probe=BLOCKED
compatibility=HOST_AND_SBF_PASS_VERUS_BLOCKED
```

The host and SBF artifact hashes intentionally differ because they are
different target code generation products. The second SBF `rlib` hash matched
the first (`d444c0ac118de1cb24d9fe6b509df7beafc1c0f1a8c2828b24e26b170da0ad1c`)
and measured reproducibility for this narrow artifact. The probe emits no ELF,
does not link an adapter, and does not run `solana-program-test`; those are
separate gates rather than inferred successes.

### Compatibility interpretation

The common executable subset is currently compatible for this probe: the same
source digest compiled under host Rust and SBF without a source edit or
target-specific economic branch. This is a falsifier against an immediate
syntax/toolchain mismatch, not proof of semantic equivalence. In particular:

1. No Verus frontend or proof result is available, so the three-way comparison
   is incomplete.
2. The SBF result is an `rlib`, not a deployable program ELF. Account parsing,
   entrypoint behavior, CPI construction, compute use, stack, heap, and runtime
   behavior remain untested.
3. Host assertions exercise only the probe functions. They are not canonical
   protocol vectors and do not establish solvency, partition, fee, or adapter
   invariants.
4. Reproducible code generation here does not prove compiler correctness or
   Solana runtime correctness.

## Exact gates

### Go gates for E1 promotion

All gates must pass and be bound to a source revision, lock digest, machine
description, and captured command output:

1. **Pin completeness:** upstream Rust, SBF builder, platform tools, Verus,
   `vstd`, Z3, Cargo dependencies, and any adapter dependencies have exact
   versions and immutable source hashes. `UNAVAILABLE` is not a pin.
2. **Single-source identity:** the Verus input, ordinary Cargo input, and SBF
   input have one identical source digest. No executable `cfg(verus_only)` or
   target-specific economic branch is permitted.
3. **Verus closure:** Verus verifies the named arithmetic, range, transition,
   and codec properties with no `unsafe`, `assume`, `admit`, axiom,
   `external_body`, `assume_specification`, proof-only public precondition, or
   release-only verifier shortcut. The theorem and assumption inventory must
   be captured.
4. **Differential vectors:** canonical and adversarial vectors pass in ordinary
   host execution, erased/verified host execution, and SBF integration tests.
   A mismatch is a stop, not a fixture adjustment.
5. **Adapter/ELF:** the exact erased source is linked through the minimal native
   SBF adapter, a reproducible ELF is produced twice, and the adapter's hostile
   byte/account checks are tested. This spike does not meet this gate.
6. **Resource envelope:** annotated and unannotated controls have recorded
   compute, stack, heap, ELF, account-data, and transaction measurements under
   disclosed safety margins. A proof pass cannot waive a resource failure.
7. **Mutation falsifiers:** changing a fee equation, range boundary,
   collateral update, or codec length makes a relevant proof/vector fail.

### No-go / redesign gates

Stop the single-source Verus/SBF architecture and narrow or redesign it if any
of the following occurs:

- Verus is missing, unpinned, or requires a different executable source;
- SBF needs target-specific economic behavior or materially different integer
  semantics;
- proof-only public preconditions leak into the unverified adapter;
- host/SBF vectors differ, malformed bytes are accepted, or a mutation leaves
  the relevant check green;
- ELF, stack, heap, compute, or transaction limits are impractical;
- proof closure requires a prohibited shortcut or an unreviewed trusted
  specification.

### Current decision

| Gate | Current result | Decision |
| --- | --- | --- |
| host common-subset compile | PASS | continue |
| SBF common-subset compile | PASS | continue |
| SBF rebuild reproducibility | PASS | continue |
| prohibited-source scan | PASS | continue |
| Verus version/proof | BLOCKED: binary absent | no E1 promotion |
| adapter/ELF/program-test | NOT RUN | no claim |
| resource and mutation matrix | NOT RUN | no claim |

The correct present-tense result is **GO for further offline probe work; NO-GO
for declaring the E1 toolchain gate closed or creating the protocol workspace**.
No engineering result here closes Gate L0 or authorizes a public-network act.
