# Strict all-eight checkpoint at 551ffc1b9 — 2026-09-08

All eight programs built from one committed source and passed the existing
checked-upgrade gate. This is a local campaign runtime checkpoint, not a
devnet deployment or proof that its accepted campaigns complete.

| Fact | Value |
| --- | --- |
| Source commit | `551ffc1b99abdf01478e7e963a1ac20a99e6c0f6` |
| Git tree | `087213b10f48169ddd810e86182ac9a7605a3b69` |
| Source-tree manifest SHA-256 | `63c3915387c7a36458d3aa7d18a42fd2e7237c315f92a6154ec42861a0d7e843` |
| Checked gate SHA-256 | `8c6357f8e2fa257d574fde4913cd67ca54b873584360b1b6d2f630f49760f621` |
| Release gate SHA-256 | `3317fd0777c60a57afd23ff69f325893770af5c1aabecf1e9aae53113c1ead81` |
| Run record SHA-256 | `4e0dd10d35de18ae018cf140aee05b91ac1a2b8707accd3c34f56655e9d7b0b6` |
| Solana CLI | `4.0.2`, source `1845f426`, feature set `6ff76655` |

Candidate root:
`hbox:/home/hbox/dclutch-strict-551ffc1b-ensemble-20260908-retry/candidate`.
Builds ran on hbox through `swarm-build`. The native bootstrap package check
also passed from a detached checkout at this source revision before publication.

Integration independently checked the bytes and SHA-256 of all 52 artifact
references in `CHECKED_UPGRADE_GATE.json`, including every ELF, build log,
frame report and checked manifest. All eight links have compilation markers,
zero SBF diagnostics and zero measured frames at or above 4,096 bytes. The
combined build log contains no containment-wrapper no-op marker.

| Program | ELF bytes | Deepest measured frame | ELF SHA-256 |
| --- | ---: | ---: | --- |
| Accelerator | 831624 | 3264 | `67e5c8865ab4cdd618c9971f090cbd5a6b60ba8397c4de895172cedd87cad8c6` |
| Claims | 1457432 | 3904 | `b18309487b59dd39745e7937fe59e6e49eb6a983b1a0f4ea6c2f2ed056039ef2` |
| Core | 1205712 | 3968 | `0df8ef6371848b87573f3c8931a2797feae45b7e1792afd1a208dcc8b847e3f9` |
| Custody | 468768 | 3968 | `b936b48790ac89ff65d36b28562674fc95f4ba4e6e0f8a763122a6c4baed6422` |
| Registry | 237696 | 1984 | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| Rent | 143152 | 1344 | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| Resolution | 957040 | 3968 | `aa1efaa57dd24d1e27725df37d32132088fa972f32697e868e3063f416ba129c` |
| Trading | 2476040 | 3968 | `add6a37bc23c52fd6e816e107d5942a7994f6e226a1507ce375432cbb252c7ff` |

This source includes the General 128 KiB profile, the funded-seal join reuse,
the Ensemble provider-route identity correction, and the Structured root-width
repair. It predates subsequent Series receipt and projected Custody fixes and
the General CPI sort-key cache. It is not the final runtime for this development
wave. A profiled Trading ELF is a separate diagnostic artifact; it must not be
substituted for the ordinary Trading ELF in this table without stating the mix.
