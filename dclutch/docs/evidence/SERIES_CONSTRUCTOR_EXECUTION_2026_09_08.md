# Series full constructor execution — 2026-09-08

These are native constructor tests, not ProgramTest, local-validator, or devnet
execution. Both named tests executed and failed. Neither is accepted-path
evidence yet.

The measured source is `551ffc1b99abdf01478e7e963a1ac20a99e6c0f6`, tree
`087213b10f48169ddd810e86182ac9a7605a3b69`. The detached checkout is
`/private/tmp/dclutch-cut-check-c3f1a152b`; its directory name records its initial
revision, while its measured HEAD was advanced to the source above. The
repository root and full HEAD were printed before each measurement. The
canonical workspace target was `/Users/ember/dev/dclutch/target`.

`cargo check -p dclutch-local-successor-bootstrap --locked` passed at this
revision. Its log is `/private/tmp/dclutch-cut-check-551ffc1b9.log`.

Each test used `cargo test -p dclutch-local-successor-bootstrap --locked --bin
dclutch-local-successor-bootstrap <name> -- --exact --test-threads=1`. The runner
executed both rows regardless of the first result.

| Exact test name | Executed result | Located boundary | Log |
| --- | --- | --- | --- |
| `series_consume_geometry::tests::full_constructor_compiles_initial_consume_profile_from_the_child_bank` | 0 passed, 1 failed, 946 filtered | The Core route constructor refuses the join between its child request and the canonical M0 Market. The specific identity conjunct still needs localization. | `/private/tmp/dclutch-551-consume-constructor.log` |
| `series_expire_geometry::tests::derive_roles_accepts_canonical_m0_and_refuses_root_basis_and_alias_substitutions` | 0 passed, 1 failed, 946 filtered | A generated alias names a physical account different from its resolved representative. The exact coordinate still needs localization. | `/private/tmp/dclutch-551-expire-constructor.log` |

Consume is owned by the Series hydrator lane; Expire by the census/route-binding
lane. Each owes a source-derived repair followed by an accepted full
constructor and its exact substitution controls. An isolated alias helper or
successful package compilation cannot replace these failed full constructors.

Runtime work is separately open: selected accelerator construction, finalized
Prepare, the Consume child walk, and two completed occurrences. No result in
this document closes those stages.
