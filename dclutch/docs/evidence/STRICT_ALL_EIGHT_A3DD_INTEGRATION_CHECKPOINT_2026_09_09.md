# Strict all-eight V2 integration checkpoint and frame baseline — 2026-09-09

## Verdict

Exact committed source `a3dd6962f6fd366e90e512ffbf15c21d89cd6541`
completed the checked genesis-candidate runner on hbox. All eight production
SBF links compiled freshly through `swarm-build`, the production diagnostic
count was zero, and the source-pinned Product handoff, checked Upgrade gate,
reproducible gate, run record, and successor campaign pack sealed.

After the runner exited, all eight link descriptors, the reproducible gate,
and the campaign pack reopened with the exact source verifiers. Two
independent production-only frame captures then agreed byte-for-byte and
admitted the baseline in `tools/gates/frames-baseline.json`.

Series selection was absent and remains recorded as `false`. General's
production CU wall and fresh runtime campaigns are separate evidence. This
record is local reproducible build and frame evidence; it is not validator,
deployment, devnet, or mainnet evidence. The baseline covers this exact
checkpoint only. Source changes after `a3dd6962...` require another
exact-source capture rather than inheriting these rows.

## Source and builder

| fact | value |
| --- | --- |
| source revision | `a3dd6962f6fd366e90e512ffbf15c21d89cd6541` |
| source Git tree | `45508616ef2bb9df686984abb85ad7c077ae26b8` |
| complete source inventory SHA-256 | `e35f38635ab86c2c9cc42823e96565a87a8ca4cff0dae675e36819ba2bd1b3dc` |
| clean hbox source | `/tank/dregg-build/dclutch-v2-integration-source-a3dd6962f` |
| sealed candidate | `/tank/dregg-build/dclutch-strict-a3dd6962f-v2-integration-checkpoint-20260909` |
| frame evidence root | `/tank/dregg-build/dclutch-frames-a3dd6962f-v2-integration-20260909` |
| builder | hbox Linux/x86_64, `swarm-build`, `SWARM_MEM_MAX=32G`, `CARGO_BUILD_JOBS=4` |
| host Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| SBF toolchain | Solana CLI 4.0.2; `cargo-build-sbf` 4.0.0; platform rustc 1.89.0; `sbpf-solana-solana` |
| root `Cargo.lock` SHA-256 | `c4a918b009363758f7ec36c346b4a54e356ed4d9c6f8347c3e6c0913f1ef244a` |
| lock-set SHA-256 | `7abf43ab4c80d0f8ff7baed0ea7327cb7a9a3f707c681e9168d11521964953a7` (one lock; immutability passed) |
| build run id | `246d4c2b74e4eb1fc942d90a2aa4cb59c1a667892a6c52272bbd36c0edc1b541` |

The runner used explicit locked package selection for every production,
profile, and frame build. Its final output records eight fresh links and no
shipped build diagnostics. The driver log has no scheduler already-loaded
marker or runner refusal.

## Production links

Each admitted ELF is an ordinary no-feature production build. Trading's
separate `hot-cu-profile` ELF is diagnostic-only and is absent from the role
manifest and release set.

| role | bytes | production ELF SHA-256 | deepest production frame |
| --- | ---: | --- | ---: |
| accelerator | 834,832 | `a3ca6421e946f29ad19f33fe6929d9d3acf353ec3389bcb6bd7fd535f6076f2a` | 3,136 |
| claims | 1,463,640 | `af929bf526ef202bec3ffffe27ad7390b7b2118344ac5e6b7646292978271205` | 3,904 |
| core | 1,174,512 | `f86ac8721985788d8e351f96319c59f68214a8ce543dcb5f1470ec170718100e` | 3,968 |
| custody | 473,608 | `d362938a1c9a16324ee6613397d77ee8bdd9dbc2b2c39a4ce4ae9af6e11bb4ff` | 3,968 |
| registry | 251,024 | `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f` | 2,048 |
| rent | 143,496 | `6d00dfde5c764c7039e58df784ec91a88aa216e513a9f5fb79815a10cc358ac7` | 1,344 |
| resolution | 1,002,320 | `967db471707e039ccb977398fc5cf92055e776934fc3c219ed2a90f6f918e424` | 3,968 |
| trading | 2,754,496 | `3cb176c1dd2c01da9c0a1dbbe5b86b14f42a82f9eb8b5f1b640870f4718f3701` | 4,032 |

The production Trading frame maximum is 64 bytes below the 4,096-byte SBPF
v0 wall. This is an admitted measurement, not reusable frame budget.

## Seals and independent reopen

| artifact | SHA-256 |
| --- | --- |
| checked Upgrade gate | `051aad59506c18b7140e417cea3818a0ff835ec47aae0fcec69a68b4761bc638` |
| reproducible release gate | `d30a4508f90d2901d89bbc2e4aaeed035390022d4dd8ee92d7103b492078f1fa` |
| release-gate run record | `6101dc8d677d3120b5668272f1c4ee800beda5da044b5f0697963dbd75b79402` |
| successor campaign pack | `4a58d8d3187d90577c61bfffb52f2b6cbf5c826261f5873edc87d5948c229df9` |
| five-role multiprogram manifest | `363a789d8536f42542da088b56b8a3a9dc4b1f4ecc58f8d91f881fb1aa26f41c` |
| genesis infrastructure manifest | `932b29e18ee3f3443ea7444732654ff3063aeeb668ba723d9093030518593650` |
| source-pinned Product bootstrap | `cf625c65d4b05a10b9efd47e4299b94bcd859d381923baa1288bd621936c6b92` |

`artifact_provenance.py verify` passed separately for accelerator, claims,
core, custody, registry, rent, resolution, and trading. The independent
reproducible-gate verification reported `link_count=8`, the exact source
revision and source digest above. `successor_campaign_pack.py verify` then
rehashed and authenticated the complete pack at the immutable candidate path.

## Independent frame admission

Capture B was a complete fresh production `tools/gate frames` run against the
clean exact-commit checkout. It compiled all eight packages with explicit
`--locked -p <package>` selection in isolated scratch targets.

Capture A used the checked candidate's seven no-feature frame objects for
accelerator, claims, core, custody, registry, rent, and resolution. Those
objects are independently bound by each verified role descriptor. Trading was
rebuilt alone without `hot-cu-profile`, with an isolated target and fresh
top-package marker. The exact source frame parser produced all eight reports,
and `frames assemble` canonicalized them. The capture-composition record SHA
is `b6836a416170e5fe0198651d4b7b6d8bdd4fa4859de4c4e731b1e34c80d9fc28`;
its object/report/log inventory SHA is
`44a96763f259a8d9d68315fdfe7d3338260eed6dc4a70f5ece111fd3965f0788`.
Every object referenced by Capture A remains at its recorded absolute path.

The independent captures are byte-identical:

```text
capture A SHA-256 = 903d616c62b15a987856482a7515e2472d8dba8801a725bcf37ec54f7470111b
capture B SHA-256 = 903d616c62b15a987856482a7515e2472d8dba8801a725bcf37ec54f7470111b
```

`tools/gate frames accept` emitted and schema validation checked:

```text
schema=dclutch-sbf-frame-baseline-v1
commit=a3dd6962f6fd366e90e512ffbf15c21d89cd6541
link_count=8
bound_bytes=4096
sha256=d05fc4b4e97e14f92f43c4d4b1cd8e5d20f64f24a5ce26f740ffc25253f0214e
```

The accepted frame counts are 51 accelerator, 310 claims, 265 core, 129
custody, 66 registry, 24 rent, 240 resolution, and 996 trading. All frame
objects and reports used for the composed capture are preserved under the
candidate and frame evidence roots.
