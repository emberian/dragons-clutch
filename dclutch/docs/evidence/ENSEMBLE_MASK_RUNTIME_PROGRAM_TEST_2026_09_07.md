# Ensemble mask runtime ProgramTest — 2026-09-07

This evidence records a ProgramTest execution after the full funding-mask
closure in commit `5b2a121b7b5f65be67778b11c4349ac8f5e6ad75`.

The current Core and Resolution ELFs were built on hbox from its checked base
`56767e555d05ecd005edec4fc939d269e8757417` with the three source files from
that commit rsynced into the build tree. This is a mixed-source build label,
not a release cohort claim. `swarm-build` native check and both SBF links had
zero matches for `Stack offset.*exceeded`, `overwrites values in the frame`,
and the scheduler no-op marker.

| Program | SHA-256 | Artifact |
| --- | --- | --- |
| Resolution | `68137e3bdc79b98df6a4b584ea55bb353f032dc1b77512a7ebeedcbfd0b182fa` | `ensemble-mask-5b2a-elf/dclutch_resolution_proof_sbf.so` |
| Core | `a08769a886cf03a061376dc5741c4141c545554830a3c9b79423477b94115c97` | `ensemble-mask-5b2a-elf/dclutch_core_sbf.so` |

The exact command was:

```sh
SBF_OUT_DIR=/private/tmp/dclutch-ensemble-mask-5b2a-programtest-elf \
  cargo test -p dclutch-svm-harness --test resolution_core_v3_lifecycle \
  a_real_pyth_member_capture_writes_a_fragment_and_keeps_primary -- --test-threads=1
```

It passed. The test loads the fresh Core and Resolution ELFs, with the other
required programs from the checked Release-B test cohort. It drives a real
Pyth VAA through the Receiver `PostUpdate` instruction and the direct
Resolution member-capture entry; the member fragment is written while the
Source remains Primary. This is ProgramTest evidence only. It does not claim
local-validator execution and it does not exercise terminal admission or
close on the new full-mask path.
