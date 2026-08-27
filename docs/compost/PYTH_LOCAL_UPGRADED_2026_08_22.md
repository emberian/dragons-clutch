# PYTH-FIXTURE-001: upgraded Pyth synthetic-local evidence

## Decision

dClutch accepts eleven immutable fixture files from Dragon's Clutch as
non-production test evidence. No Dragon's Clutch Rust source, Source/Feed
account, action router, instruction-sysvar parser, or workflow graph crosses
the repository boundary.

These fixtures execute captured upgraded Pyth receiver and router programs,
their real ABI, and real signature verification on local Solana
infrastructure. Their price message is synthetic. They are not devnet price,
provider-availability, production-release, or mainnet evidence and cannot
populate `PRODUCTION_RELEASES`.

## Source and provenance

- Source repository: `dragon's-clutch`
- Source commit: `169a1bad530d1d62b55c11acf39fa285a1740cb0`
- Source path:
  `programs/clutch-sbf/svm-tests/tests/fixtures/real-pyth-local/`
- Upstream repository: `pyth-network/pyth-crosschain`
- Upstream commit: `f50a3faf9fc5a223a22889799b2f778900f186b3`
- Upstream license: Apache-2.0; the original complete license and attribution
  remain beside the bytes as `UPSTREAM_LICENSE`
- Destination: `fixtures/pyth/local-upgraded-2026-08-22/`

The original `PROVENANCE.md` is retained verbatim beside the bytes. It records
the capture procedure, toolchain, provider addresses and ProgramData slots,
synthetic message contents, reproducibility limitation, and complete-account
hashes.

## Accepted invariant and new owner

The retained invariant is narrow: dClutch can test its new internal-CPI
resolution path against the captured real provider programs and cryptographic
verification, including rollback after a successful provider post.

The fixture directory owns the immutable external evidence. The fresh
`dclutch-pyth-svm` crate owns hostile provider/loader decoding;
`dclutch-pyth-contract` owns funding, receipt, and instruction semantics; the
future SBF program will own account authority, CPI ordering, and rollback.

## Exact files and SHA-256

| File | SHA-256 |
| --- | --- |
| `PROVENANCE.md` | `2ac2344d5c5a2b0470349fcce305a23218ece64343277ae83f5d8c897481c874` |
| `UPSTREAM_LICENSE` | `814162e3e1ec1c02ab68400bf98859ad73af3d67e19c026e98426a91085973a1` |
| `guardian-set-0.account.hex` | `f1b139a3e279943758a39da80a64a0115a5c7d11640bc8579eee9256f77ec146` |
| `price-update.account` | `e5435e5b2e54d6083a9d1230e33f0635f6c74eb9db62899cfbb559f99c798a2b` |
| `receiver-config.account` | `05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa` |
| `receiver-initialize.data` | `d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f` |
| `receiver-post-update.data` | `3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d` |
| `receiver.so` | `c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64` |
| `router-initialize.data` | `3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b` |
| `router.so` | `f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb` |
| `signed.vaa` | `ed8b973f36a932b9ec88659953859c8096f14e5aebd085bbe32b22c41a142c0d` |

## Deliberately rejected assumptions

- No Instructions-sysvar adjacency authority; dClutch owns post-then-consume
  inside one instruction.
- No SourceSpec, Feed head, archive, sealing, or mock provider authority.
- No claim that the synthetic feed identifies a real asset.
- No production release row derived from pre-cutover fixture bytes.
- No reliance on the old transaction builder or account graph.
- No private guardian keys; only the already signed public message is retained.

## Required fresh tests

The new test harness must independently verify every file hash before use,
authenticate both Program-to-ProgramData links and deployment slots, execute
the captured router and receiver ELFs, require the full 134-byte price update,
and prove that a dClutch refusal after provider CPI rolls back provider and
dClutch writes atomically. Those tests are fresh implementations and may not
copy the old harness.

## Amendments

### 2026-08-27 — digest drift corrected, and the fixture dated against the clusters

Two record errors, both in this file rather than in the bytes:

1. `PROVENANCE.md` was amended after transplant to document
   `guardian-set-0.account.hex`, and this file still recorded the pre-amendment
   digest `636e590b02585c98e55ad8603bf06d03c7df2426a1816958f8eae2dffca2fd87`.
   The table now records the current digest, which is the one
   `crates/dclutch-svm-harness/tests/support/pyth_provider.rs` has been
   enforcing all along. No fixture byte changed.
2. The eleventh file, `guardian-set-0.account.hex`, was missing from the table
   and from the "ten files" count. Both are corrected.

Separately, a bounded public-RPC observation on 2026-08-27 dated these bytes
against the live clusters. The result matters for how this fixture is
described:

- `receiver.so` and `router.so` are **byte-identical to the live upgraded
  receiver and Wormhole receiver on both `mainnet-beta` and `devnet`**, after
  the 2026-08-26T16:00:49Z Pyth Core cutover. The directory name
  `local-upgraded-` is accurate and was accurate before the cutover: this
  capture named the upgraded program IDs and took the upgraded binaries.
  Nothing in the ABI moved and the adapter needed no change.
- The devnet `ProgramData` complete-body digests and deployment slots recorded
  in `PROVENANCE.md` still reproduce the live devnet accounts exactly.

What is **not** cluster-real in this directory, and must keep being labelled
synthetic:

| file | status |
| --- | --- |
| `receiver.so`, `router.so` | real, and currently live on both clusters |
| `receiver-config.account` | **lab only** — chain 1 / emitter `[0x01; 32]`, fee 1, `minimum_signatures = 5`; the live Config admits Pythnet, fee 0, `minimum_signatures = 3` |
| `guardian-set-0.account.hex` | **lab only** — nineteen synthetic upstream guardians; the live set is five Pyth keys |
| `signed.vaa`, `receiver-post-update.data`, `price-update.account` | **lab only** — synthetic feed `[0x2a; 32]`, no asset meaning |
| `router-initialize.data`, `receiver-initialize.data` | **lab only** — the initialization that produced the lab Config and guardian set |

The lab's 5-of-19 quorum is a lab shape. It is not a scaled model of the live
3-of-5 and must not be described as one. What the fixture proves is that the
real router ELF performs real signature verification and that dClutch rolls
back atomically around a successful provider post.

The cluster-observed counterpart is `fixtures/pyth/upgraded-2026-08-26/`, which
holds the third program of the generation (the push oracle, whose id change
moved every per-feed account address), the per-cluster `ProgramData` headers,
and the live `Config`, `GuardianSet[0]`, bridge config and SOL/USD accounts on
both clusters. It deliberately does **not** duplicate `receiver.so` or
`router.so`; the byte equality is the recorded fact instead. See
`docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md` §Supersession.
