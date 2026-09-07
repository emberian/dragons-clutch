# Economics and Custody local-validator campaign — 2026-09-07

Owner: CODEX-ECONOMICS. Evidence level: bounded local-validator execution.
This is neither devnet evidence nor a release claim. The validator and every
ephemeral signer were local and were removed by the wrapper at completion.

## Source and runtime identity

The campaign host was the clean detached hbox checkout at
`9ab52277f556e2ec081c7d195e9ec358ed46c409`. That revision supplies the
campaign producer and wrapper. It is deliberately distinguished from the
runtime programs: those came from the checked release gate's archived source
revision, `56767e555d05ecd005edec4fc939d269e8757417`.

The checked gate was
`hbox:/tank/dregg-build/dclutch-codex-release-b-20260907/CHECKED_UPGRADE_GATE.json`,
SHA-256
`6031115cd75da111c10d248a8489d7179d8edd5d320bc59e024a5f76e7107909`.
The wrapper's `source.json` records this explicit classification:
`diagnostic-host-current-runtime-checked`. It is a host-current diagnostic of
the identified checked runtime; it does not assert that the host revision was
the source of the runtime ELF set.

The checked substrate and economics commands ran through `swarm-build` on
hbox with `SWARM_MEM_MAX=32G`, `CARGO_BUILD_JOBS=6`, on loopback port 47386.
The checked substrate accepted its activation sequence before the economics
stages began. The campaign run root is
`hbox:/tank/dregg-build/dclutch-codex-economics-20260907/run-47386`.

## Finalized transactions and poststates

The transcript has six declared stages: three accepted and three deliberately
refused. The eight evidence transactions span slots 1328 through 1552.
Refusal transactions are finalized transactions whose exact custom error is
shown below; their protected account states were reread byte-for-byte by the
campaign after refusal.

| Case | Outcome | Finalized signature / slot | Exact result |
| --- | --- | --- | --- |
| Parameters Found by hostile authority | refused | `5dpDqWtkp3BzVUnm9f15RUD3xrJKLEVMGL13X3AEQCwFRiJCicfX2FYoG9a1r6onQhH68joiSescpUHnLm7SvDnB` / 1360 | `ProtocolParametersSbfErrorV1::FoundingAuthority`, `Custom(24836)` |
| Parameters Found by retained Custody upgrade authority | accepted | `JkpHYQN3Hvby7v5TMH8kMXWnoMn3aSxkUXh4i6cjQiMWjheac79tZzpbnRGcpXWwAh9UpnwQrxs5eEkrdccopxq` / 1392 | Record `Ffw8EZCEhFkFagh7kb8RY1cg2uEyy6b5GjyUGAjnL3Ah` owned by Custody `FfeKNyYMztvB8VGViXqkgWyNaLP8XTbJ3FFMBtsqpn5A`; governance authority `FaeGKG8azf5nDKYYJH8qjekm3gc7zRYyTcyXvuWeBvJ4`; pending proposal false; protocol beneficiary default; protocol take 0; hoard principal moved 0. |
| Parameters Found duplicate | refused | `bXHcAFTcaHdQemCfvHSRkZfuJVcj5pu2E3vcffqusnJDGiC2tGuiuoGEWak2BRLfpzBtmxJwsQ5XKLaRAJ9CKFv` / 1424 | `ProtocolParametersSbfErrorV1::Record`, `Custom(24834)` |
| Upkeep Found | accepted | `4euavoT8nvCbfit3n6CGw1c3cNKM96t4RuDcMRHNDrPJ9MKT1otjZEyrHzqNciqT2H7oQYNtgtHFzQD5Xs1ub4ep` / 1488 | Created vault `7QK8qFHzmis6kQEMqZ9qCYooryegbZcvoyEaavgjSopA`; founding rent 1,781,760 lamports. |
| Upkeep Deposit Credit, 17,001 lamports | accepted | `2dmw5w3bdWLCD6Z1nF1NaVSnofkVi4QaA1ekurhWh8fQMfpr7qHM5Dk4SnyUNExGJYRWGbDRaKNrJuudqiczZHPX` / 1520 | `creditAmount=17001`, `creditClass=upkeep:deposit`, `creditCount=1`, `depositInflow=17001`, `donationInflow=0`, `inflowTotal=17001`, `unreceiptedLamports=0`, `donationRemainderProduced=false`, `hoardPrincipalMoved=0`, vault lamports after credit 1,798,761. |
| Upkeep Found duplicate | refused | `2dUsbV6uZPWWUjgt3DUA3ZPWg9XbFoYwryR3QdFdwn54zKHEP9Xz6o1cPS3JfnrsmjyPQfKQd6Z7MY6nrZV7RZo1` / 1552 | `UpkeepVaultSbfErrorV1::Vault`, `Custom(25090)` |

The two accepted upkeep signatures establish an actual Found followed by a
nonzero Credit on the checked Custody runtime. The named poststate excludes
Hoard principal from the credit and records no donation remainder; it is
therefore evidence of the intended upkeep economics partition, within this
bounded run.

## Immutable artifacts

All hashes are SHA-256 and were computed on hbox after the transcript declared
`completed: true`; the wrapper had then removed the local validator.

| Artifact | SHA-256 |
| --- | --- |
| `source.json` | `22a2c605ca6ea63dc2a1026b7ae60102f92ac7e344c5b808e5f4ffffa0733c11` |
| `transcript.json` | `7a5c5e4da47d8c540dc600c5031835edcb414b28af5c49d0c67aba01fa9b2424` |
| `campaign/evidence.json` | `20de6ea65f63d3c678e624af2e399a95adbd51b1e8ca83285890877bad345b95` |

`transcript.json` is the campaign authority for the named stage outcomes and
poststates. `campaign/evidence.json` retains the eight finalized transaction
records, including the raw `InstructionError` structures for all three
refusals. `source.json` binds the current host, checked runtime and checked
gate identities above.
