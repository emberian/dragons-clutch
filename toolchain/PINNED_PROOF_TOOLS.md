# Pinned proof tools

Retrieval date: **2026-08-18**. Platform: **aarch64-apple-darwin** (Apple
Silicon, Darwin 25.6.0). Machine-readable form: [`versions.env`](versions.env).

This file records *what is installed and pinned*. It records no verification
result. Every proof-content claim in this repository still has to be earned by
a named, non-vacuous theorem that a pinned tool actually checked; see
[Current gate status](#current-gate-status) for what these tools report today,
which is not a passing proof.

## Verus

| Field | Value |
| --- | --- |
| Release tag | `release/0.2026.08.15.7d4628a` |
| Version | `0.2026.08.15.7d4628a` |
| Commit | `7d4628a8543d3e51e6e314c52032c9bab43f0f53` |
| Published | 2026-08-17T01:08:24Z (newest non-prerelease at retrieval) |
| Artifact | `verus-0.2026.08.15.7d4628a-arm64-macos.zip` (284,555,156 bytes) |
| Artifact sha256 | `a9d03889dfbf841c92565be605cd857b4171943dac564b1f4c00ddcda9830ada` |
| Install prefix | `~/toolchains/verus-0.2026.08.15.7d4628a/verus-arm64-macos` |
| `verus` binary | `~/toolchains/verus-0.2026.08.15.7d4628a/verus-arm64-macos/verus` |
| `verus` sha256 | `07b3859fc335fd9bf803323baf82584dcefb62d74329561875353f5fde93fe8b` |

The published GitHub asset digest and the locally computed digest of the
downloaded zip agree; the artifact was not re-hosted or repacked.

### Rust frontend toolchain

The release reports `Toolchain: 1.97.1-aarch64-apple-darwin`, matching
`rust-toolchain.toml` at commit `7d4628a`. `rust_verify` links
`@rpath/librustc_driver-e03ed1db822dfa4c.dylib`, which is satisfied by the
already-present rustup toolchain `1.97.1-aarch64-apple-darwin`.

**This is not the host build toolchain.** The repository builds with
`1.89.0-aarch64-apple-darwin` (`HOST_RUST_TOOLCHAIN`). No Verus release exists
that uses a 1.89.0 frontend and is anywhere near current; the two pins are
independent and must not be conflated. Any claim that Verus "verified the code
the host compiler compiles" has to account for the fact that a different rustc
front end parsed and type-checked it.

### Bundled solver

The release ships its own `z3` next to the binary and prefers it over `PATH`:

* bundled `z3`: `Z3 version 4.16.0 - 64 bit`, sha256
  `edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`
* `source/tools/get-z3.sh` at commit `7d4628a` pins `z3_version="4.16.0"`
* the separately installed system `z3` at `/opt/homebrew/bin/z3` is also
  4.16.0, so the two agree, but the bundled copy is the one in use

### vstd

`vstd` is shipped prebuilt inside the release (`libvstd.rlib`, `vstd.vir`) and
has **no independent revision of its own** — it is pinned only transitively by
the Verus commit. Recorded digests:

* `libvstd.rlib`: `91f842643674660a1da8799eda3eceaa37e5816682d026b3d860bc59a81be61c`
* `vstd.vir`: `3061321483841360c760fa3fb38b22be57533052cf2100eb51956aa31205fa7d`
* `libverus_builtin.rlib`: `8b105db7052551af7340e4507ef160f62c9a2275d1509334102a167a94617ab9`

### Install method

Downloaded over HTTPS from the project's own GitHub release, digest-verified
against the digest GitHub publishes for the asset, unpacked into a user prefix.
No `sudo`, no system path, no package manager, no build from source.

```sh
curl -fL -o verus-0.2026.08.15.7d4628a-arm64-macos.zip \
  https://github.com/verus-lang/verus/releases/download/release/0.2026.08.15.7d4628a/verus-0.2026.08.15.7d4628a-arm64-macos.zip
shasum -a 256 verus-0.2026.08.15.7d4628a-arm64-macos.zip
# a9d03889dfbf841c92565be605cd857b4171943dac564b1f4c00ddcda9830ada
unzip -q verus-0.2026.08.15.7d4628a-arm64-macos.zip -d ~/toolchains/verus-0.2026.08.15.7d4628a
```

The binary is not on `PATH`. `toolchain/scripts/run_verus.sh` resolves it from
the recorded prefix, overridable with `VERUS_PREFIX`. To use it interactively:

```sh
export PATH="$HOME/toolchains/verus-0.2026.08.15.7d4628a/verus-arm64-macos:$PATH"
```

The bundled `macos_allow_gatekeeper.sh` was **not** run; it is unnecessary for a
`curl` download (no `com.apple.quarantine` xattr was set) and the script has a
literal defect anyway (`${{BASH_SOURCE[0]}}`, a leftover template brace).

## Rocq

| Field | Value |
| --- | --- |
| Version | `The Rocq Prover, version 9.2` (release 9.2.0) |
| Install method | Homebrew, `homebrew/core` formula `rocq`, stable 9.2.0, revision 0 |
| Bottle | `arm64_tahoe`, rebuild 0, sha256 `2e2a4c16e90c21d3baa45352ff39df6bc934d643c5e9fecdddf9f4ee3483aecd` |
| Upstream source | `https://github.com/rocq-prover/rocq/releases/download/V9.2.0/rocq-9.2.0.tar.gz` |
| Upstream sha256 | `a45280ab4fbaac7540b136a6b073b4a6db15739ec1e149bded43fa6f4fc25f20` |
| Prefix | `/opt/homebrew/Cellar/rocq/9.2.0` |
| Binaries | `/opt/homebrew/bin/rocq`, `/opt/homebrew/bin/coqc` |
| Built with | OCaml 5.5.0 |

Rocq is the renamed Coq; the Homebrew formula lists `coq` as an old name and
still installs a `coqc` shim, which is why `rocq/check.sh` finds a tool on
either branch.

```sh
brew install rocq   # poured from the arm64_tahoe bottle, exit 0
```

Dependency versions resolved at install time: `gmp 6.3.0`, `ocaml 5.5.0`,
`ocaml-findlib 1.9.8_4`, `ocaml-zarith 1.14_5`.

## Current gate status

### `toolchain/scripts/run_verus.sh` — runs, and **fails**

The script no longer refuses on "Verus is not installed", because a reviewed
pin now exists. Every other refusal was kept and three new ones were added:

| Condition | Exit | Behaviour |
| --- | --- | --- |
| no Verus at the pinned prefix and none on `PATH` | 2 | original `BLOCKED: verus is not installed…` message, unchanged |
| resolved Verus version != pinned version | 3 | refuses to report an off-pin run as evidence |
| resolved Verus frontend toolchain != pinned toolchain | 3 | refuses to run |
| probe source sha256 != pinned digest | 4 | refuses to run against an unreviewed source |

The pinned probe digest is
`10b2087683d3c2cb423768eb9c612c00ea929b171835c15d3d16792d6b8b19ac` for
`toolchain/probes/no_std_core/src/lib.rs`. The script previously *printed* this
digest without comparing it; it now compares and refuses on mismatch.

With the pin satisfied, Verus **rejects the probe**:

```
error: Error: The verus_builtin crate was not imported but it is necessary to run Verus.
You likely need to add `use vstd::prelude::*;` at the top of a lib.rs or main.rs file.
```

This is a real result, not a harness problem. The probe is plain Rust with no
Verus annotations, and Verus will not process a crate that does not import its
builtin. `--no-vstd` produces the identical error. Making it pass requires
editing the probe source, which changes the pinned digest — so the correct
state is a recorded failure, not a green gate.

### `rocq/check.sh` — reports `status=PASS`, and that PASS proves nothing

`rocq compile` accepts `rocq/ClutchKernel.v` with two warnings
(`deprecated-from-Coq` on the `From Coq Require Import` line, and
`abstract-large-number` on `U64_MAX`). Neither is an error.

**The file contains no theorems.** Its six obligations are
`Definition <name> : Prop := <statement>` — unproved statements whose
*types* elaborate. `PASS` therefore means "the definitions typecheck", and
carries zero proof content. The file says so itself in its header comment. It
is honest, but it must never be cited as verification.

Independently, `successful_transition_is_well_formed` contains a **vacuous
conjunct** at line 426:

```coq
(forall s o, resolve s o = Some s -> state_validb s = true)
```

`resolve` requires `active_b s = true` and returns a record with
`st_phase := Resolved`, so `resolve s o = Some s` is unsatisfiable and the
conjunct holds trivially. Every sibling conjunct correctly binds a fresh `s'`.
This is an output-shape defect in the statement, not in the model. It was left
unmodified — fixing a specification while installing tools would hide it.

This vacuity is not a reading of the source; it was machine-checked against the
pinned Rocq 9.2, and `Print Assumptions` reports `Closed under the global
context` (no axioms). To reproduce, place this beside `ClutchKernel.v`:

```coq
From Coq Require Import Arith.PeanoNat Lists.List.
Import ListNotations.
Require Import ClutchKernel.

Theorem resolve_conjunct_is_vacuous :
  forall s o, resolve s o = Some s -> False.
Proof.
  intros s o H.
  unfold resolve in H.
  destruct (state_validb s) eqn:Hv; simpl in H; try discriminate.
  destruct (active_b s) eqn:Ha; simpl in H; try discriminate.
  destruct (Nat.ltb o (length (st_payouts s))) eqn:Hl; simpl in H;
    try discriminate.
  unfold finish in H.
  destruct (state_validb _) eqn:Hv2 in H; try discriminate.
  injection H as H.
  assert (Hp : st_phase s = Resolved) by (rewrite <- H; reflexivity).
  unfold active_b in Ha. rewrite Hp in Ha. discriminate.
Qed.
```

The correct statement almost certainly binds a fresh output, matching its five
siblings: `forall s o s', resolve s o = Some s' -> state_validb s' = true`.
That edit belongs to whoever owns the specification, not to this pin record.

### Verus shadow specs — all three fail to compile

Run directly with the pinned binary
(`verus --edition 2021 --crate-type=lib <file>`); none has ever been checked
before, and none was modified.

| File | Result |
| --- | --- |
| `verus/kernel/lib.rs` | 2 × `E0308`: `Seq::subrange` expects `int`, got `nat` (lines 31, 32) |
| `verus/accumulator/accumulator.rs` | 4 × `E0308`: `int`-typed sums returned from `u64`/`u128` spec fns (lines 71, 75, 79, 83) |
| `verus/batch/batch.rs` | `cannot find macro 'verus' in this scope` — the file has no `use vstd::prelude::*;` |

`verus/batch/batch.rs` additionally carries four `proof fn`s whose entire
postcondition is `ensures true`. Those are vacuous placeholders, self-labelled
as `TODO`, and would prove nothing even if the file compiled.

## What remains unpinned

* **vstd revision** — no independent version; pinned only transitively by
  `VERUS_COMMIT`. If a future run mixes a source-built vstd with this binary,
  that pin is lost.
* **Homebrew formula provenance** — this Homebrew installs from the JSON API,
  not a git tap checkout, so there is no `homebrew-core` commit to record. The
  formula version, bottle sha256, and upstream tarball sha256 above are the
  strongest available pin. An opam switch pinned to `rocq-prover.9.2.0` would
  give a reproducible build record instead of a trusted prebuilt bottle; that
  is the upgrade path if bottle trust becomes unacceptable.
* **Rocq stdlib / dependency closure** — pinned only by the formula's own
  dependency resolution at install time, not by a lockfile.
* **`librustc_driver-e03ed1db822dfa4c.dylib`** — supplied by the ambient rustup
  toolchain `1.97.1-aarch64-apple-darwin`, not by the Verus artifact. The
  dylib hash in the link table matches, but the toolchain is outside this
  repository's control.
* **Correspondence to the executable Rust** — entirely unpinned and unproved.
  The shadow specs are hand-written restatements; nothing checks that they
  describe `crates/clutch-*`.
