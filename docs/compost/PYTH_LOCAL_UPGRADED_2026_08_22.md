# PYTH-FIXTURE-001: upgraded Pyth synthetic-local evidence

## Decision

dClutch accepts ten immutable fixture files from Dragon's Clutch as
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
| `PROVENANCE.md` | `636e590b02585c98e55ad8603bf06d03c7df2426a1816958f8eae2dffca2fd87` |
| `UPSTREAM_LICENSE` | `814162e3e1ec1c02ab68400bf98859ad73af3d67e19c026e98426a91085973a1` |
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
