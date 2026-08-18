# Verified bytecode paths: could the deployed artifact refine a Lean model?

Status: **research scouting only.** No code was modified and nothing is committed.
This authorizes no deployment, toolchain change, or E0 decision. Every citation was
retrieved **2026-08-18** unless dated inline.

**Short answer.** For the SVM, not as a whole-program path and not soon — though
the bottom of the chain now exists, because the sBPF ISA has a real mechanized
semantics (Isabelle/HOL, OOPSLA 2025) with a Lean 4 port. For the EVM the whole
chain is genuinely close to real, but the fee economics say we do not want an EVM
target. The cheapest thing that buys real assurance is neither: it is a Lean model
of the **account and authorization plane**, because that is where every P0 we have
ever found lived, and a correct compiler would have caught none of them.

---

## 1. What sBPF/SVM bytecode is

**ISA.** Eleven 64-bit registers — `r0` return, `r1`–`r5` args, `r6`–`r9`
call-preserved, `r10` frame pointer, `r11` stack pointer — plus `pc`. "Bytecode is
encoded in 64-bit slots, and instructions can occupy one or two slots, indicated by
the op code of the first slot"
([Anza, *The Solana eBPF Virtual Machine*, 2026-08-12](https://www.anza.xyz/blog/the-solana-ebpf-virtual-machine)).
Memory regions sit at fixed virtual bases (stack `0x200000000`, program input
`0x400000000`).

**The 4 KiB frame and 64-frame depth are VM properties, not compiler flags.**
`maxCallDepth = 64`, `stackFrameSize = 4096`
([`Solanalib/SBPF/State.lean`](https://github.com/solana-foundation/leanprover-solanalib/blob/main/Solanalib/SBPF/State.lean));
SIMD-0166 (*Implemented*) confirms the VM "reserves 4096 bytes of stack space for
each function frame" and swaps the static partition for an explicit stack pointer
while leaving the total stack budget unchanged
([SIMD-0166](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0166-dynamic-stack-frames.md)).
Agave's `DEFAULT_STACK_FRAME_SIZE = 4_096` matches; SBF additionally forbids
`std::thread`/`std::sync`/`std::time` and static writable globals
([limitations](https://solana.com/docs/programs/limitations)).

**Versioning is a moving target.** The version lives in the ELF `e_flags`
(SIMD-0161, superseded by SIMD-0500). v2 changed encoding — `LDDW` removed,
`HOR64` at `0xF7`, `CALLX` reading its target from the src register, load/store
opcodes relocated
([SIMD-0173](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0173-sbpf-instruction-encoding-improvements.md),
*Review*). v3 = SIMD-0178 (static syscalls) + SIMD-0189 (stricter ELF headers) +
[SIMD-0377](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0377-ebpf-isa-compatibility.md)
(JMP32 class, new opcodes, realigning with the LLVM eBPF backend; *Review*,
2025-10-09).
[SIMD-0500](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0500-disable-deployment-of-sbpf-v0-v1-v2.md)
(*Idea*, 2026-03-17) would ban deploying v0/v1/v2. Any emitter we write inherits a
standing maintenance obligation to this stream.

**Loader, verifier, metering.** `deploy_program!` loads the ELF strictly, runs
rBPF `verify`, and reloads under the current runtime environment; failures cache as
`FailedVerification` ([program-execution](https://solana.com/docs/core/programs/program-execution)).
The VM "checks for CU meter overflows at every instruction" via
`ContextObject::consume` (Anza), but consensus CU accounting is *basic-block
granular*, and an irregular abort mid-block charges the transaction its **entire
requested** CU
([SIMD-0182](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0182-conditional-cu-metering.md),
*Implemented*) — so cost is a function of bytecode *plus a consensus rule that has
already changed once*.

**Runtime state model.** Five account fields (lamports, data, owner, executable,
rent_epoch). "Only the account's owner program can modify its data or debit
lamports"; "any program can credit lamports to any writable account"; rent-exempt
minimum `(size + 128) × 3,480 lamports/byte-year × 2`
([accounts](https://solana.com/docs/core/accounts)). CPI: "account privileges
(signer, writable) extend from caller to callee. A callee cannot escalate
privileges"; `invoke_signed` has the runtime derive PDA pubkeys and add them to the
valid-signer set *before* privilege checking; stack depth 5 (9 with SIMD-0268);
indirect reentrancy A→B→A returns `ReentrancyNotAllowed`
([CPI](https://solana.com/docs/core/cpi)). On each frame pop the runtime
re-verifies lamport balance and readonly protection.

**None of that state model is in the bytecode.** It lives in the loader, the
serializer, the syscall implementations, and the invoke stack.

## 2. Formal semantics for eBPF/sBPF

- **CertSBF — "A Complete Formal Semantics of eBPF Instruction Set Architecture
  for Solana."** Yuan, Zhang, Lu, Sanan, Chang, Zhao, *PACMPL* 9 OOPSLA1 art. 80,
  April 2025, [doi:10.1145/3720414](https://dl.acm.org/doi/10.1145/3720414).
  Small-step Isabelle/HOL semantics of the binary-level ISA; assembler/disassembler
  with a consistency proof; a model of the Solana verifier with a step-safety lemma
  (6.4); an x86-64 JIT correspondence track. Repo
  [shenghaoyuan/CertSBF](https://github.com/shenghaoyuan/CertSBF), ~60 `.thy` files,
  last push 2026-04-29, validated by OCaml extraction against rBPF's own
  `execution.rs` suite and randomly generated instructions. **Live work, not a
  one-off paper artifact.**
- **[solana-foundation/leanprover-solanalib](https://github.com/solana-foundation/leanprover-solanalib)**
  (Apache-2.0, created 2026-06-02, last push 2026-07-01, 12 stars). Self-described
  "purely experimental — exploring framework shape"; "lives entirely at the spec
  layer — it doesn't run on-chain." `Solanalib/SBPF/` is a **Lean 4 port** of the
  OOPSLA 2025 semantics: all 22 instruction forms, decoder, small-step interpreter,
  verifier with `step_ne_err`. `Account/` and `Instruction/` are modelled "1:1 with
  the real Rust crates (`solana-pubkey 4.2`, `solana-account 4.3`,
  `solana-instruction 3.4`)." Its README claims validation via a harness `spinoza`
  ("tens of thousands of randomized programs … with zero divergences"), whose
  `lift` subcommand "emits a deployed program's `.text` as an importable `BpfBin`
  term." **`github.com/lgalabru/spinoza` returns 404 as of 2026-08-18**, so that
  validation claim is not checkable by us. The roadmap lists **"Refinement from
  `SBPF/` up to the spec layers" as open**: "the pieces exist at both ends; the open
  work is the connecting theorem."
- **Prior eBPF work:** CertrBPF (Coq rBPF interpreter with CompCert C extraction;
  end-to-end mechanized proof of a JIT-accelerated eBPF VM,
  [CAV 2024](https://www.irisa.fr/prive/talpin/papers/cav24.pdf)) and Jitterbug
  (Rosette/SMT per-instruction JIT equivalence — the one such artifact whose output
  runs in the Linux kernel).
- **Nobody models syscalls.** The Lean interpreter resolves calls through a
  *function registry*; grepping `Interpreter.lean` at `main` finds no syscall
  handling at all. Our own ELF imports `sol_try_find_program_address`, `sol_log_`,
  `sol_memcpy_/memcmp_/memset_`, `sol_panic_`, `abort`, and — since the Token-2022
  leg — `sol_invoke_signed_rust` (SBF_BRINGUP.md). Every one is an axiom, not a
  semantics, in every model above.
- **[Certora Solana Prover](https://www.certora.com/blog/sol-formal-verification)**
  (2023-08-08, now open source) is the one production-grade tool consuming SBF
  directly — "the first automatic verifier (both academic and commercial) that can
  analyze SBF programs directly." It proves *properties* under an SMT/abstract-
  interpretation stack; it is not a refinement proof against a Lean model, and its
  public material states no soundness caveats, which is itself a caveat.

## 3. Formal semantics for the EVM, especially in Lean 4

Markedly more mature, and specifically in Lean:

- **[EVMYulLean](https://github.com/NethermindEth/EVMYulLean)** (Nethermind,
  EF-funded) — executable Lean 4 EVM + Yul model for Cancun passing **99.99%
  (22,330/22,332)** of the official Cancun execution tests, "the most complete
  formal model of the EVM to date"
  ([2026-02-06](https://www.nethermind.io/blog/a-trustworthy-formal-model-of-evm-yul-in-lean));
  gaps are Yul-side (no gas, no CREATE/CREATE2, partial SELFDESTRUCT). Feeds
  **Clear**, an interactive Yul/Solidity verification framework.
- **[powdr-labs/yul-compiler](https://github.com/powdr-labs/yul-compiler)** — "a
  formally verified Yul to EVM bytecode compiler" in Lean over `yul-semantics` and
  `evm-semantics` (all three pushed 2026-08-18). Theorem: if the compiler accepts a
  Yul program, the compiled bytecode computes exactly what the Yul semantics say,
  under a gas bound ([2026-07-03](https://powdr.org/blog/yul-compiler)). WIP: no
  optimizers, not all Yul features.
- **[argotorg/EquiVM](https://github.com/argotorg/EquiVM)** (created 2026-06-04,
  pushed 2026-08-07) — foundational **refinement** proofs in Lean that deployed
  bytecode refines a `Sol⁻` spec, already applied to MakerDAO Dss, WETH9 and
  OpenZeppelin contracts across `solc` versions and optimizer settings. This is the
  translation-validation shape, done for real.
- **[evm-smith](https://leonardoalt.github.io/evm-smith)** (2026-05-04) —
  hand-written bytecode plus a Lean 4 solvency proof, no compiler in the chain:
  "the deployed artifact is bytecode + proof." Scale marker to keep: **86 bytes of
  bytecode, ~24,700 lines of Lean.**
- Non-Lean: **KEVM** ([jellopaper.org](https://jellopaper.org/), validated against
  40k+ EVM test programs) and **Kontrol**, used commercially by Runtime
  Verification on production protocols including Optimism.

On the EVM the full chain — Lean semantics → verified compiler → bytecode, and
separately bytecode → refinement proof — is not hypothetical in 2026. On the SVM
only the bottom exists.

## 4. Compilation paths out of Lean

**Confirmed: Lean 4 compiles to C against a heavy runtime.** "A C file is produced
for each Lean module; these are then compiled to native code using a bundled C
compiler"
([reference](https://lean-lang.org/doc/reference/latest/Elaboration-and-Compilation/)).
Memory management is reference counting
([reference](https://lean-lang.org/doc/reference/latest/Run-Time-Code/Reference-Counting/)),
with `lean_object` a boxed header carrying `m_rc`, allocation via
`lean_alloc_small_object` over malloc/mimalloc, RC updates using `<stdatomic.h>`
([`lean.h`](https://github.com/leanprover/lean4/blob/master/src/include/lean/lean.h)).
`src/runtime/` is a large C++ library (`object.cpp` ~99 KB, `apply.cpp` ~75 KB,
`thread.cpp`, `mpz.cpp`, `libuv.cpp`); Lean 4.30.0 (2026-05-26) raised Lean's own
stack to 1 GB; an LLVM backend exists in-tree but is not the default. The only
empirical small-target port found — Lean on an ESP32-C3 — had to disable Lean's
allocator ("Lean's runtime allocates 8 MiB segments"), stub `initialize_Init` with
an admittedly unsafe no-op, and drop floats and threading
([2024-07-31](https://kuruczgy.com/blog/2024/07/31/lean-esp32/)). Against SBF's
4 KiB frames, no threads and no writable globals, **this is a runtime rewrite, not
extraction**, and no Lean→SBF attempt exists anywhere (unsourced negative).

Also: **Lean's own compiler is in the TCB and unverified** — the FAQ says the TCB
"is extended with the Lean compiler …, the Lean runtime …, and the code generation
backend" ([faq](https://lean-lang.org/faq/)) — and `native_decide` is currently
capable of proving `False` ([lean-pitfalls](https://github.com/nielsvoss/lean-pitfalls)),
which is why Mathlib bans it. Routing trust through Lean's codegen is *worse* than
what we have.

**What actually ships is always the same pattern**: author a small first-order DSL
*inside* the prover, prove the DSL→target translation, keep the prover's runtime
out of the artifact. Production: **CompCert** (Coq, C→asm; Airbus for over a
decade, IEC 61508-3 SIL 3, and qualified for the ATR 42/72 MFC_NG in early 2026);
**Low\*/KaRaMeL** (F\*→C; HACL\*/EverCrypt in Firefox NSS, the Linux kernel,
WireGuard); **Fiat-Crypto + Bedrock2** (Coq→C/asm, inside BoringSSL, i.e. Chrome);
**Jasmin** (compiler proven to preserve semantics *and* constant-time; libjade).
Research-grade: **CakeML**. Extraction-to-subset: **ConCert** (verified MetaCoq
erasure → Liquidity, CameLIGO, a Rust subset). Emit-then-validate:
**Jitterbug/Serval**, whose TV found "16 previously unknown bugs … in five other
deployed JITs," all upstreamed to Linux
([OSDI '20](https://www.usenix.org/system/files/osdi20-nelson.pdf)); **Alive2**.

**In smart contracts, nobody has shipped mainnet bytecode out of a verified
compiler.** DeepSEA ([arXiv:2405.08348](https://arxiv.org/abs/2405.08348)) is
dormant — last substantive commit 2021-03-30. ConCert's Concordium extraction repo
is archived (2021-06-30); Scilla is archived (2025-05-20). Mi-Cho-Coq/Dexter2 is
hand-written-then-verified, and a Trail of Bits audit found inter-contract exploits
Mi-Cho-Coq structurally could not detect. Move Prover verifies; it does not
generate. The live counterexample is **[Verity](https://github.com/lfglabs-dev/verity)**
(LFG Labs, MIT, created 2026-02-08, pushed 2026-08-18): "a formally verified smart
contract compiler written in Lean 4", a Lean EDSL compiled EDSL → CompilationModel
→ IR → Yul with a machine-checked theorem per layer, zero `sorry`, one axiom — and
**Yul→bytecode is `solc` 0.8.33, pinned and trusted**. Its
[TRUST_ASSUMPTIONS.md](https://github.com/lfglabs-dev/verity/blob/main/TRUST_ASSUMPTIONS.md)
says "verification is scoped rather than total": constructors, fallback/receive,
events, typed errors, proxies, external calls and non-trivial loops sit outside the
proven fragment. No mainnet deployment sourced.

## 5. Economics: is an EVM target even worth wanting?

One transaction writing ~4 KB of state plus ~64 orders of fixed-width arithmetic
(ETH $1,912.13, SOL $76.89, CoinGecko, 2026-08-18):

| Chain | Recurring fee/tx | Refundable deposit |
|---|---|---|
| Solana | **$0.0004 – $0.0019** | **$2.26** — `getMinimumBalanceForRentExemption(4096)` = 29,399,040 lamports |
| Ethereum L1 | **$0.51 – $0.91** *today* (**$29–$205** at 5–20 gwei) | none |
| Base | **$0.035 – $0.062** | none |
| Arbitrum One | **$0.116 – $0.207** | none |

L1 gas is 3.02M–5.37M: 21k intrinsic + 16.4k calldata + **2.83M** for 128 cold
`SSTORE_SET` ([EIP-2929](https://eips.ethereum.org/EIPS/eip-2929)) + arithmetic.
Today's 0.079 gwei L1 base fee is historically anomalous; the Solana priority-fee
sample over 150 slots was **median 0**. Solana is **~20–100× cheaper than the
cheapest L2** here and 300×+ cheaper than L1 even today. The EVM roadmap is
repricing *against* our shape:
[EIP-8007](https://eips.ethereum.org/EIPS/eip-8007) (Draft, Glamsterdam, target Q4
2026) would move `GAS_STORAGE_SET` 20,000 → 97,920 and the calldata floor 10/40 →
64/64 — our 128-slot write goes 2.83M → ~12.5M+ gas, **~4.4×**, inherited by every
OP-Stack and Arbitrum chain — and
[EIP-8037](https://eips.ethereum.org/EIPS/eip-8037) would price state at 1,530
gas/state-byte. On Solana **nothing about rent or base fees has activated**:
SIMD-0084 removed periodic rent collection but not the deposit, SIMD-0553 has no
activation date, SIMD-0436/0437 are *Idea* and superseded.

**Verdict.** The mature Lean-EVM tooling is real tooling for a chain whose fee
structure is hostile to a 55-account, 4 KB-write, batch-clearing protocol. **Do not
chase the EVM to reach the proofs.** The proofs do not port to us; the costs do.

## 6. The two paths, honestly costed

**Path A — verified compiler from a tiny Lean DSL to sBPF.** Verity's leverage
comes from stopping at *Yul* and trusting `solc`: someone else built and maintains
the last mile. On sBPF there is no Yul. We would own the DSL, the IR, every
translation proof, the sBPF encoder, the ELF emitter, the relocation and `e_flags`
story, and the maintenance obligation to SIMD-0173/-0178/-0189/-0377/-0500 as they
land — with SIMD-0377 still in *Review*, so the encoding we prove against is not
frozen. Worse, the DSL fragment must be pure and **our program is not**: the ELF
imports `sol_try_find_program_address` (8–9 calls per instruction, *dominating* the
cheap families' cost), the `sol_mem*` family, and `sol_invoke_signed_rust`. A
verified DSL→sBPF compiler would prove things about the arithmetic and leave every
runtime call an axiom. Cost: multi-person-year, ongoing. Assurance against our
actual defect classes: near zero.

**Path B — emit, then translation-validate against a Lean-formalized ISA.** Keep
the Rust→SBF build; lift the deployed `.text` into Lean and prove it refines a Lean
transition model. This is legitimate here **precisely because bytecode has a formal
semantics** — the thing Rust does not have, and the reason "translation validation
of a Rust AIR against a spec" is a lie. The pieces exist: `Solanalib.SBPF` for the
semantics, `spinoza lift` for the ELF→Lean term, EquiVM as an existence proof on
real contracts.

Two things kill it at our size. **Scale**: evm-smith needed ~24,700 lines of Lean
for 86 bytes of bytecode; our reproducible ELF is **332,368 bytes** across eight
instruction families. Discount rodata and dead code heavily and the ratio still
does not survive. **Re-proof cost**: a TV proof is per-artifact, so every dependency
bump, `Resolve` optimization, or terms-account revision invalidates it. The one
thing that could change this arithmetic is EquiVM's explicit bet — "refinement
proofs are intended to be generated by autonomous LLM agents" — which is being
*evaluated*, not established.

**Path C, which the question did not name and which is cheaper than both.**
Charon compiles Rust to LLBC; **Aeneas** translates it to a pure lambda calculus
with a **Lean backend** ([AeneasVerif/aeneas](https://github.com/AeneasVerif/aeneas),
917 stars, pushed 2026-08-18). `leanprover-solanalib`'s roadmap names exactly this
as "the complement to the artifact-level sBPF route": `cargo charon
--preset=aeneas`, `aeneas -backend lean`, prove `theorem rust_impl_refines_spec`.
That closes the *first* arrow in ARCHITECTURE §10 — Lean model ↔ actually shipped
Rust — with a theorem instead of shared vectors, leaving rustc/LLVM/SBF where they
already are: named, trusted, reproducibility-checked.

## 7. The scope point: verified bytecode with an unmodeled runtime

Every P0 repaired during the adversarial review
(docs/implementation/ADVERSARIAL_REVIEW_V0.md §2) is a **missing predicate**, not a
mistranslation:

- ineligible-fill acceptance — `validate_fills` never required an ineligible fill
  to be zero;
- canonical-allocation verification — `verify` accepted any fill vector meeting
  loose bounds while `propose` built a canonical one;
- settlement domain / consideration / paired orders — no cumulative per-order
  ledger, so quantity could be re-consumed (class: **replay, authorization**);
- vertical error atomicity — state mutated before `check_conservation()`, with the
  invalid prestate constructible through the public API;
- prefix resolution before maturity — resolution with no maturity or sealed state;
- reference materialization of an unaccounted claim — no check reconciling a
  position's internal + external balances against `KernelAccount.total_supply`.

A verified compiler proves *emitted ≡ source model*. **Every one of those would have
compiled perfectly.** So verified bytecode with an unmodeled runtime buys the
absence of miscompilation and of frame/UB hazards, and nothing else we have
actually been bitten by. The UB half is not nothing — the SBF backend reports
over-4 KiB frames as non-fatal `Error:` lines that do not fail the build, which is
why `run_bringup.sh` greps for them; eight such functions exist today in
`clutch-solana-layout`/`-reference`, all dead-code-eliminated from the ELF.

**Modelling the runtime in Lean is the work that would matter, and it is a real
project.** Present: account record with ownership/writability, instruction +
`AccountMeta` with signer/writable flags (solanalib has both, 1:1 with the Rust
crates). Missing: the loader's serialization of accounts into the VM input region;
the per-frame pop check on lamport balance and readonly protection; PDA derivation
(a SHA-256 model, since `sol_try_find_program_address` is our dominant syscall); CPI
privilege propagation with the no-escalation rule; depth-5 stack and
`ReentrancyNotAllowed`; transaction atomicity. Nobody has published this; scope is
plausibly comparable to CertSBF itself.

**Our differential harness is the right empirical check on such a model and is the
strongest asset we hold here.** SBF_BRINGUP records **52 account comparisons, 52
matches, 0 mismatches** — every writable account byte-identical to the oracle
post-state across eight families executing in a real local Agave bank — plus a
*falsification step* in the same validator session (flip one byte of the Hoard
collateral expectation and it goes red). That is structurally `spinoza validate`
for the runtime rather than the ISA. A Lean runtime model is a **conjecture about
the bank**; the differential is how the conjecture gets tested; and the model is
worth exactly the differential's coverage — today eight families, one fixture, two
outcomes, three observations, explicitly "not an envelope."

## 8. Ranked recommendation

1. **Model the account/authorization plane in Lean and restate the adapter's checks
   as theorems.** Signer presence, non-aliasing, ownership, replay monotonicity,
   lamport and per-outcome `total_i` conservation across a transition. Reuse
   `Solanalib`'s `Account`/`Instruction` shapes rather than reinventing them.
   *Cost:* weeks, inside the ~1,000-line Lean tree that already exists. *Buys:*
   proof pressure on the plane where 100% of our P0s lived. *Does not establish:*
   that the deployed ELF implements those checks — that arrow stays open and must
   be labelled open.
2. **Point the existing SVM differential at that model** — make the oracle
   Lean-derived rather than a second Rust implementation, keeping the 52-comparison
   harness and its self-falsification step. *Cost:* moderate. *Buys:* empirical
   fidelity of the model to a real bank. *Does not establish:* anything outside the
   fixtures; extend coverage before extending claims.
3. **Track `leanprover-solanalib` instead of building ISA semantics.** Apache-2.0,
   Solana-Foundation-owned, already carries the OOPSLA port. *Cost:* low. *Caveat:*
   experimental, last push 2026-07-01, and its validator `spinoza` is a 404 — treat
   its differential claim as unverified. Contributing our harness as the missing
   public validator is a cheap way to make the dependency safe to lean on.
4. **Aeneas/Charon spike on one pure kernel function** — `cargo charon` → `aeneas
   -backend lean` → one `rust_impl_refines_spec` against the existing Lean kernel
   model. *Cost:* bounded spike with a clean kill criterion. *Buys:* a real
   refinement arrow from shipped Rust to Lean. *Does not establish:* anything about
   rustc, LLVM, or the SBF backend.
5. **Only after 1–4: a translation-validation probe on the smallest family.**
   `feed-advance` (7,673 CU, 419 tx bytes) is the only plausible candidate. Lift
   `.text`, attempt refinement against the Lean transition. Research with a stated
   abort condition, not a plan.
6. **Do not build a Lean→sBPF verified compiler, and do not add an EVM target.**
   The first is multi-person-year work proving the wrong thing; the second is
   20–100× more expensive per transaction on a roadmap actively repricing against
   our workload.

## 9. What I could not source

- Any Lean hello-world binary-size figure (only unattributed forum numbers).
- Any attempt anywhere at Lean→SBF or Lean→eBPF codegen.
- The `spinoza` harness (404 on 2026-08-18), so solanalib's "tens of thousands of
  randomized programs … zero divergences" is unverifiable by us.
- Whether Certora's SBF verifier has published soundness caveats — none appear
  publicly, which is not the same as there being none.
- Any deployed mainnet contract address produced by DeepSEA, ConCert, or Verity.
- Team size or elapsed time behind Verity and EquiVM, so every cost estimate in §6
  is an inference from artifact scale, not a sourced number.
