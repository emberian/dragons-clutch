# Strict all-eight native completion candidate — 2026-09-09

## Verdict

Exact committed source `a8e3b4e8c593a5a63eef94a51375f46ba5ad75d2`
completed the checked genesis-candidate runner on hbox with exit 0. All eight
production SBF links were freshly compiled through `swarm-build`; the shipped
diagnostic count was zero; all eight source-pinned frame reports remained at
or below the 4,096-byte bound; and the checked Upgrade gate, reproducible gate,
run record and successor campaign pack sealed. Independent reopen verification
of the reproducible gate and campaign pack passed after the runner exited.

This candidate incorporates the General, Structured and Claims native changes
that followed the admitted `fd7e5fec...` frame baseline. Its runner frame
reports are build evidence, not a new two-capture baseline admission. Series
selection was absent and is recorded as `false`. This is a local reproducible
release candidate; it is not validator execution, deployment, or chain
evidence.

## Source and sealed handoff

| fact | value |
| --- | --- |
| source revision | `a8e3b4e8c593a5a63eef94a51375f46ba5ad75d2` |
| source Git tree | `e01b11d43fbc598f80d135b6af22298c38760fec` |
| complete source inventory SHA-256 | `77345f940cf89bb5c02469b6e16d77ba4d9d5bebb05cee87b449b47777ad0d74` |
| clean source checkout | `/tank/dregg-build/dclutch-v2-completion-source-a8e3b4e8c` |
| sealed candidate | `/tank/dregg-build/dclutch-strict-a8e3b4e8c-v2-native-completion-20260909` |
| checked Upgrade gate | `46af94470e79e86d7ceeb456574baddf1ea4c8bc705f944273b34e2df91dc90a` |
| reproducible release gate | `75d07a91b44103883eb87b9102b0692bd96c37d379b55a50cefc98c34ba364b3` |
| release-gate run record | `03d84e225cc54d5fb12872ec3aedd4b415570b41f405f21c90c36ec617131f5c` |
| successor campaign pack | `0d2442e660b7ff4ad27b7e27e1db58bf4ed992777745ad47e358bf76ad19920d` |
| five-role multiprogram manifest | `230aa4ab7f391cfc198624a21e1b37e84259f7f22654ae9b623496b123fedecb` |
| genesis infrastructure profile v2 | `a728c6f4ada7fb55f1cd5bd92a4208b5c3156aa1f449f647f9c793bd528e7c9c` |
| infrastructure manifest | `e38ec763b1bac4c6eb7bd33bbe0d61b64d81eda563606783005e7fceda3e7eda` |
| Product bootstrap | `product-handoff/dclutch-local-successor-bootstrap`, SHA-256 `a4a4ff62ed05a9c2537a75a98a2dc65a7af03ae89b393a79674b0358eecb5ee4` |
| root `Cargo.lock` SHA-256 | `c4a918b009363758f7ec36c346b4a54e356ed4d9c6f8347c3e6c0913f1ef244a` |
| lock-set SHA-256 | `7abf43ab4c80d0f8ff7baed0ea7327cb7a9a3f707c681e9168d11521964953a7` (one lock; immutability passed) |

The builder was hbox Linux/x86_64 through `swarm-build` with
`SWARM_MEM_MAX=32G` and `CARGO_BUILD_JOBS=4`. The toolchain was rustc 1.97.1
for host work, Solana CLI 4.0.2, `cargo-build-sbf` 4.0.0, and Solana platform
rustc 1.89.0 for `sbpf-solana-solana`. The persistent driver log contains no
`Unit run-u` scheduler no-op marker and no runner refusal.

## Production ELFs and frames

The runner's `sbf_shipped_feature_suffix` is unconditional and empty: each
admitted ELF is the ordinary production build with no Cargo feature.

| role | bytes | ELF SHA-256 | deepest runner frame |
| --- | ---: | --- | ---: |
| accelerator | 832,640 | `cf60df159c94942f959d66dd4ae963dee64713cbe79734e19e182f2db4ccfb1b` | 3,136 |
| claims | 1,463,552 | `ed5cc2b3b97d927564774bc1887f6689b2d90aeedd5fcd4089bb7c90d2ace183` | 3,904 |
| core | 1,174,368 | `d242da365eac4e7f6cea239b2649d2dd9ab6d358bf38be55974ccb09f30a492d` | 3,968 |
| custody | 470,400 | `c8072930ad813a19ba8355d38fd41857d97a78990812a0e52757ccc9ba124d6c` | 3,968 |
| registry | 251,024 | `48ae0e74a7146dedf4af46c6b97a66a93425ad3d87447f6eae8d0e016fc1608f` | 2,048 |
| rent | 143,496 | `6d00dfde5c764c7039e58df784ec91a88aa216e513a9f5fb79815a10cc358ac7` | 1,344 |
| resolution | 998,664 | `cf976a6008d2adaa722acf4b04e03179e35df90d792391dfc19dc4810fc29247` | 3,968 |
| trading | 2,741,600 | `82d35f87a1086542bd8e87a6e32e8010a1bcb3961791e78081154c22c97e0529` | 4,032 |

The separate Trading CU diagnostic, SHA-256
`3bb85c0549df7e01ba4a6aaa248a0f188ead9dd33c77d59c8c4b608e7f6ea317`,
alone carries `--features hot-cu-profile`. It is measurement-only and is not
admitted by a role manifest or the release set. Trading's runner frame report
uses this diagnostic build; the shipped Trading ELF remains feature-free.

Structured and Claims runtime lanes consume the immutable ELF, gate and pack
paths above and record their validator evidence separately. Any later host
binary compiled into `host-target` is downstream runtime support and does not
change the sealed candidate artifacts.
