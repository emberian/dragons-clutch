# Strict all-eight checkpoints — 2026-09-08

Two clean, committed-source all-eight checked-release candidates were built on hbox through `swarm-build` with `SWARM_MEM_MAX=32G` and `CARGO_BUILD_JOBS=6`. Each first passed the offline successor-bootstrap native check, then freshly linked all eight SBF roles and rebuilt the role frame measurements. The diagnostic scan rejected scheduler no-op markers and both SBF stack-frame diagnostic forms.

| checkpoint | exact source | source tree SHA-256 | checked gate SHA-256 | reproducible gate SHA-256 |
| --- | --- | --- | --- | --- |
| Dealer/Ensemble | `687431e0e913b835929657c568b87715a4119bb9` | `63b7ea2aa0d1363c46c4aad357f9c9fb6ee11a785bde76da3113323e9ce673fa` | `f1cd43493bb5dc5ed9738847040e130d1fb8ada348866ff5470d0d525674e68f` | `d74c81d8e76280086f0b1fc6277dc2b031c192bffd9c9d6e2af6153d2608d321` |
| General closure | `68153eefc98a5d8160fd8ed683426d6d12b7a8be` | `5fcb3e6b10d1318b8e121a70a54336c1e6848cf2d3b9513d574f1a12f15d22e5` | `9e38413c7fce1640c35d0e6683084c5a6e5ca19c09735be8abd46866781cdd57` | `fc6f2884429b6d9aa4acb20724f7a4fdf9c60b817f05d246fcde0f19e40c56a8` |

Artifacts are under `/tank/dregg-build/dclutch-strict-687431e0e-20260907/candidate` and `/tank/dregg-build/dclutch-strict-68153eefc-20260907/candidate`. Both candidates recorded eight fresh links, zero SBF diagnostics, and zero frames at or above the 4096-byte limit; provenance, reproducible-gate, and successor campaign-pack verification passed.

The General checkpoint shipped Trading `d7972a8b2fd2a4d575a2aebf60c702bd9ec0b4823067a1cac136d10e24328e23`, Claims `12abf6b1a7e276e2181697b58cccd47051550a6ab3e36d11f00c090f462e042b`, Resolution `346cf7bc9c00ab1854a38ee4826b84a40c1be163d51a362756a58519e315f550`, and Accelerator `8f19d86e1d47014a99654aa68a2ba03336da9879168a91cd53f5f52241029a44`.

These are offline diagnostic gates. No final `tools/gate frames` pair was captured, and this evidence does not claim selected Series accelerator runtime coverage, deployment, or a release cut.
