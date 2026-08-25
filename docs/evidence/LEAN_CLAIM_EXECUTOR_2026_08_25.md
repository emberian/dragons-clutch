# Lean-generated claim executor and controller composition — 2026-08-25

> Historical artifact evidence. Its combined pairwise projection and qedsvm
> path theorem were superseded by the canonical replay/Position owner model in
> [`COMPILED_SIGNED_DIRECT_2026_08_25.md`](COMPILED_SIGNED_DIRECT_2026_08_25.md).
> The measurements below remain exact for their named source commit and bytes.

## Scope

This evidence concerns the claim-only exact-account executor and experimental
controller relay at source commit `884fe2a`. Lean derives four replay/claim
effects and two indivisible custody transfers from one admitted Direct frame.
The executor implemented here owns only the four claim effects; collateral is
absent from its 80-byte state and 72-byte instruction.

This is not a complete Direct successor. Signed-intent admission, exact
controller ProgramData authentication, real Realm-selected token custody, and a
machine-checked composition from the high-level theorem to the ELF theorem are
still open.

## Reproducible artifacts

The exact commit was reconstructed with `git archive 884fe2a` and built with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-claims-proof-sbf/Cargo.toml \
  --lto --optimize-size --dump
cargo build-sbf \
  --manifest-path programs/dclutch-controller-proof-sbf/Cargo.toml \
  --lto --optimize-size --dump
```

The build used cargo-build-sbf 4.0.0, platform-tools v1.53, SBF rustc 1.89.0,
and emitted zero verifier diagnostics.

| Program | ELF bytes | SHA-256 | Loader V3 capitalization |
|---|---:|---|---:|
| claim executor | 1,872 | `229f399d457d494bf5629545794edeee984a6c0437bad0293c4ff12fc4ad9569` | 0.015374640 SOL |
| SDK controller experiment | 14,312 | `42f843670281a9139dfe7b5283132e5358961c4a563682bdbd1dfed07c29f8e2` | 0.101957040 SOL |
| combined | 16,184 | — | 0.117331680 SOL |

Capitalization uses `Rent::default()`, a 36-byte Loader V3 Program account, and
45-byte ProgramData metadata per program. It excludes custody and registry
programs, transient buffers, state accounts, and transaction fees.

The claim executor's `.text` is 984 bytes / 119 decoded instructions with
SHA-256
`a13251120085644b991d07c2290680d0e0b26cc46fcf4cbdcb69b27b0023aaf4`.
It has no `.rodata` or `.data` section. Fresh output from Lean 4.30.0's
`EmitClaimSbfProfile.lean` exactly equals the checked-in Rust constants.

Compared with the retired seven-effect exact target, claim specialization
reduces state from 104 to 80 bytes, the plan from 120 to 72 bytes, the ELF from
2,232 to 1,872 bytes (16.13 percent), and success from 155 to 110 CU (29.03
percent). More importantly, it removes the false physical model in which SPL
collateral appeared as three executor-owned integers.

## Real-SVM evidence

`claims_proof_target.rs` loaded the exact claim ELF with
`solana-program-test` 4.2.1 and no native processor or mock adapter.

- canonical claim plan: 110 CU and exact two-nonce/two-claim post-state;
- eleven hostile frames refused, including privilege, owner, canonical-byte,
  outcome, conservation, replay-overflow, and late credit-overflow cases;
- every refusal restored the complete projection byte-for-byte.

`controller_authority_membrane.rs` loaded the exact optimized controller and
claim ELFs together:

- direct nonsigning-PDA impersonation refused: 7 CU;
- controller-PDA relay and successful child: 3,810 CU;
- controller journal mutation followed by late child overflow: refused at
  3,692 CU with both accounts restored byte-for-byte;
- wrong controller bump refused: 2,109 CU.

This is real runtime evidence for PDA lending, CPI composition, and transaction
rollback. It is not a proof of the Solana runtime and does not authenticate the
currently deployed bytes behind the controller program ID.

## qedsvm theorem

qedsvm v0.11.0 at commit
`2356bc6865ed36a454d2a7285bd3989518ddd31f` independently executed the exact
claim ELF at 110 CU. Its 110-PC trace has SHA-256
`e946482b668996008e20d365cfecb2267f857916c569d19fd52c0faf1039958f`.

`qedlift` emitted a 1,168-line Lean module embedding the exact `.text` bytes.
Lean 4.30.0 checks both the raw output and the stored copy, which strips only
trailing whitespace, with no `sorry`, axioms, `admit`, `external_body`, or
assumed specifications. The stored file SHA-256 is
`94bd1bbe0f9c26b8e7cdf2285ea8a0e1a13031792e40a653503835e085507a2c`.

`DclutchClaimsProofSbfLifted_lifted_spec` is an assumption-heavy,
successful-path `cuTripleWithinMem` theorem with a 109-step bound from PC 0 to
PC 118. Runtime CU (110) and theorem steps (109) are intentionally recorded
separately. This is not whole-CFG coverage, a loader proof, or the still-missing
refinement arrow from concrete projection bytes to
`Physical.claim_plan_refines`.

## Succession result

The active seven-effect exact proof target and its harness were removed in the
same source commit. Their pinned historical evidence remains reproducible from
the old commit, but they are no longer an alternative authority path.

The next gate is physical custody: a controller-derived 40-byte, two-transfer
plan must execute against real Realm-selected SPL accounts under the maker's
replay-root delegate, and a custody failure must roll back both the claim child
and any controller mutation. Only then should signed-intent admission and exact
immutable-controller release authentication be joined to the vertical slice.
