# Claims admission vacancy witness — 2026-09-09

## Change and boundary

ProtocolPosition Admit keeps the canonical Position and Admission PDA seeds
and bumps returned by vacancy authentication, then uses them in the existing
System allocation/assignment CPIs. It no longer repeats the same two PDA
searches in `allocate_pair`.

The private witness binds the exact program id, common account references,
allocation width, both seed objects and both canonical bumps. Between witness
creation and use, the route only builds candidate bytes and receipt bytes;
there is no account mutation or CPI. Account keys and the executing program id
cannot change during this frame. System still validates the supplied signer
seeds for every allocation/assignment. Existing privilege checks, both exact
PDA address checks, both System-owner checks, empty-data checks and prepaid
lamport floors remain in place. No wire or persisted layout changes.

## Controls

An isolated current Registry V2 source archive at
`/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/source` carries this
Claims-only change over `7eb4d69ada25fc8386110e666a511b8c6b27fb51`, using its own
workspace-root target. `cargo check --locked -p dclutch-claims-sbf --lib`
passed (`check-v2-vacancy.log`).

The filtered native run `protocol_position_v2::tests::admit_vacancy_` passes two
tests (`native-v2-vacancy.log`):

- The witness's two carried signer seed arrays reproduce the exact Position
  and Admission account keys; the carried account references and width are
  those authenticated by vacancy checking.
- Ten hostile inputs refuse with `ProtocolPositionSbfErrorV2::Position`: each
  wrong account key, each wrong account owner, each nonempty account, each
  underfunded account, zero width, and a different requested Position owner.
  The honest control has donated lamports above the declared floors.

For the red control, only the Position address comparison was omitted in the
isolated source. The hostile control failed at mutation 0: `Ok(())` instead of
`ProtocolPositionSbfErrorV2::Position` (exit 101,
`native-v2-vacancy-red.log`). After restoring the accepted source, both tests
passed again (`native-v2-vacancy-restored.log`).

## Real SBF diagnostic artifacts

General owns the full transaction continuation. Its warm source remains on
Registry V1; this lane applied only its exact Claims module diff, leaving that
cohort intact. These artifacts are diagnostic SBF ProgramTest evidence, not
devnet or current Registry V2 cohort evidence.

Evidence root on hbox:
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/`.

The baseline was rebuilt before applying the witness against the same warm
dependency closure. Its ELF is byte-identical to the accepted preceding cache
optimization profile ELF. Native check completed before SBF builds; both the
instrumented and production witness builds completed successfully.

The paired real-SBF runs use the same terminal-only Trading profiler:
`program-test-delegated-v30-claims-vacancy-baseline.log` and
`program-test-delegated-v31-claims-vacancy-witness.log`.

| Phase | Baseline | Witness |
| --- | ---: | ---: |
| Allocation, candidates checkpoint to allocated checkpoint | 16,165 CU | 7,944 CU |
| Release authentication | 15,074 CU | 15,074 CU |
| Product/Core authentication | 34,685 CU | 34,682 CU |
| Complete instrumented Claims Admit CPI | 99,242 CU | 89,640 CU |

Allocation saves 8,221 CU. The checkpoints are 124,381 to 108,216 CU remaining
for the baseline and 126,457 to 118,513 for the witness. Both pairs of System
allocation/assignment CPIs and the complete Claims admission succeed. The
whole-CPI difference includes generated-identity/PDA-search variation in other
phases; only the allocation phase directly measures the removed repeated
searches. This evidence does not claim the complete General lifecycle passes;
General owns the production continuation with this ELF.

ELF SHA-256:
- `claims-vacancy-baseline-profile-elf/dclutch_claims_sbf.so`:
  `7e2d765d58d621aba2c50e30827b3f8008ecea3f761c809fa1afe77d3cc4c11d`
- `claims-vacancy-witness-profile-elf/dclutch_claims_sbf.so`:
  `9e8d0b286adc6a623717455411c235cb7f1912f901cf75b73f32172697606fa7`
- `claims-vacancy-witness-production-elf/dclutch_claims_sbf.so`:
  `d30b59bd3e10518f96c97c8b805d61d2af90c6b950c8c5d503f05b72999e1b82`

Exact warm module snapshot SHA-256:
- `claims-vacancy-before.rs`:
  `526a7d8bbf388593e191feecfad6b69b210d105b92ffdc3beab03b4d583881b2`
- `claims-vacancy-after.rs`:
  `ce93217df65a307c132f97072611fd6396e30e80a5f2639b27ff0171a526b36d`

No frame baseline was captured by this lane; the implementation commit leaves
the frame ratchet red for convergence.
