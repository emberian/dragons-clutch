# Structured Claims real-ELF ProgramTest, 2026-09-07

This is ProgramTest evidence only. It neither starts a validator nor creates a
cluster transaction, and makes no devnet or mainnet claim.

## Run identity

The named target was run serially on hbox:

```text
SBF_OUT_DIR=/tank/dregg-build/dclutch-codex-evidence-20260907/structured-claims-programtest-20260907/sbf
DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR=.../program-test-evidence
CARGO_BUILD_JOBS=6 cargo test -p dclutch-claims-sbf \
  --test rational_representation_v2_program_test -- \
  --test-threads=1 --skip claim_check::
```

`claim_check::` is a separate campaign with separate bindings, so excluding it
keeps this evidence fold within `claims-rational-representation-v2`.

* source root: `/tank/dregg-build/dclutch-codex-20260907`
* source HEAD: `56767e555d05ecd005edec4fc939d269e8757417`
* actual working-source patch:  `source-working.patch`, SHA-256 recorded in
  `verification.txt` beside the run
* target source SHA-256:
  `a4411d1eb24202c0e0bf2db1c6718c0b9a5c463a26db3463c89d98ee5aaf46d7`
* target result: **31 passed, 0 failed, 18 filtered**
* folded evidence: 196 transactions; SHA-256
  `4323bb9c0f4190fe5bd247951f7683adacddbeb1a35a16908a0b303e2b562b2f`
* `claims-rational-representation-v2` witnesses: **10 checked, 0 failed**.

The retained hbox evidence root is
`/tank/dregg-build/dclutch-codex-evidence-20260907/structured-claims-programtest-20260907`.
It contains the target log, folded evidence, source patch, provenance, ELF
verification and the checked gate digest.

## Checked artifacts

The eight production ELF copies match
`/tank/dregg-build/dclutch-codex-release-b-20260907/CHECKED_UPGRADE_GATE.json`
byte-for-byte. That gate records zero SBF diagnostics and zero frames at or
above the 4096-byte limit for every link. The canonical Token-2022 v11 ELF is
also authenticated by the test fixture.

| artifact | SHA-256 |
| --- | --- |
| Claims | `19a3be2fcb625bf56166f8b638adba2e7d970421d1a08e9fdd1bc63d7ddaef5d` |
| Core | `7013924151b5fbd1f3d5abf0c88fb6199adaf2a6ba7f2a91da89014c4cf086a9` |
| Custody | `178bb636165036ef061dc2268868c845fed47e3f9bdd1632d2c06161b242a5b7` |
| Registry | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| Resolution | `4a9ca791a06b7daf47460b99de7f12ebaa9b49c6dbef2bb9e1843276e1591a39` |
| Rent | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| Trading | `27988083edccb86cb190e1317eece27478a586e425155133a75082a48d2fdffd` |
| Accelerator | `1fd892a8ad19baa55a046b263fd8f07286beed5a8fc1e967dbfeef67155776bb` |
| Token-2022 v11 | `e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697` |

The test-only Rational V2 caller is not a release program and is absent from
the gate. It was built alone by `swarm-build` into the evidence directory;
its SHA-256 is
`57bd33244ecefb590ef659fa8b8f0a34e025d8ed1d051b13f65692606dd118b3`.
Its build log has zero stack-frame diagnostics.

## Accepted sequence

| action | packet bytes | compute units | asserted state change |
| --- | ---: | ---: | --- |
| IssueStructured | 1005 | 281443 | replay revision 0→1; receipt Mint supply rises by 1 and actor receipt balance is 1 |
| UnwrapStructured | 1005 | 275947 | replay revision 1→2; receipt, shard Mints and token accounts return exactly to their pre-issue images |
| Denominate | 857 | 245629 | replay revision 2→3; one native claim moves into the selected custody Position and exactly 7 shard atoms are minted |
| Reconstitute | 857 | 235982 | replay revision 3→4; the selected custody quantity returns to its prior value and shard accounts/Mints return to the pre-denominate images |
| RedeemTerminal through Trading Hot | 1137 | 778489 | replay revision becomes 1; Claims and Custody both report success, proving the collateral-moving child returned through Trading |

The test separately proves representation redemption in `Retiring`: it burns
seven selected shard atoms, decrements the selected custody Position by one,
and leaves Structured custody unchanged. Those are nonzero poststates, not a
success-log-only assertion.

## Census observation

The fold is a complete, separate campaign evidence document for
`claims-rational-representation-v2`. The witness check above is the local
observation gate. It was not appended to a shared ledger in this run because
the available hbox evidence root supplied an inventory but no owned ledger; a
later convergence owner must invoke `census observe` against the intended
ledger under its lock. This preserves the evidence without treating a
read-modify-write ledger mutation as an unowned side effect.
