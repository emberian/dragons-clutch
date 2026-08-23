# Current unsealed SBF snapshot — 2026-08-22

Status: **local unsealed engineering evidence only**. The second-pass default
runtime input closure was clean and committed at
`169a1bad530d1d62b55c11acf39fa285a1740cb0`; commit `5ab10b0` later extended a
test campaign without changing program sources. This is not a release,
deployment candidate, signed manifest, authorization to fund or deploy, or an
independent security review.

The audited default is a production-inert profile. Its sole source-release row
is a fabricated off-curve V2 fixture used to exercise exact ingestion and
custody in local banks; that provider address cannot be deployed on a real
cluster. It has no production source release. Mock V1 and captured-Pyth
laboratory campaigns compile separate, explicitly non-production ELFs.

## Exact second-pass artifact

The complete offline audit built the default profile three times from fresh
targets, including once with a relocated Cargo home. All three stripped ELFs
were byte-identical.

| fact | value |
| --- | --- |
| ELF bytes | `2,082,320` |
| ELF SHA-256 | `193c08723eaefeff9a1c2aa53c9e3feb58960a919fb0bbb7ca5da3bd817aa95b` |
| tracked source-closure files | `129` |
| source-closure SHA-256 | `5ec6c61c2134c7e50c220ff0325d454a98b16c3bd0830acd33d28e05d8f29e03` |
| source commit | `169a1bad530d1d62b55c11acf39fa285a1740cb0` |
| profile | default; one unreachable fixture release; no production release |
| Solana CLI | `4.0.2` |
| `cargo-build-sbf` | `4.0.0` |
| platform tools | `1.53` |
| pinned Rust | `1.89.0` |

Reproduction command:

```sh
CLUTCH_SBF_AUDIT_KEEP=1 \
  programs/clutch-sbf/audit/audit_artifact.sh
```

The dependency closure contained 101 packages: 88 verified registry archives,
12 first-party packages, and one vendored package. The audit checked the exact
ten-symbol syscall surface, ELF segment and entry shape, loader sizing, and the
final-LTO stack image. The source-closure digest is the audit's path-ordered,
per-file-digest fold over the 129 tracked build inputs.

### Stack classification

The pinned backend emitted diagnostics for 28 dependency symbols while
compiling intermediate objects. None survived final LTO. The final audit found:

- `.text`: `1,942,200` bytes;
- final text symbols: `1,078`;
- unique/disassembled text regions: `1,075`;
- direct `r10` stack references inspected: `66,127`;
- deepest direct `r10` offset: exactly `4,096` bytes;
- out-of-frame direct `r10` references: `0`.

`Intent::encode` and `Intent::encoded_len` are absent from the final ELF after
the CreateMarket decoder stopped re-encoding an already parsed message. Shared
semantic field validation remains.

## Artifact-specific execution matrix

The following local runs completed after freezing the runtime closure. These
are bank and loopback-validator results, not production-source or mainnet
evidence.

| profile/campaign | exact ELF | result |
| --- | --- | --- |
| default production-inert bank | `193c0872…`, `2,082,320` bytes | `165 passed`, `0 failed` |
| `non-production-mock-source` bank | `342fdfcb0e6b0836ec9ecd492d9a8577c87f493b49fd8c35e3cb47c448d06112`, `2,110,240` bytes | `168 passed`, `0 failed` |
| `non-production-real-pyth-lab` R2 bank | `38442c94c4ce25c18e8487f551a427e553b17db0d48cb31324b47a8a299ff902`, `2,084,264` bytes | `10 passed`, `0 failed` |
| keeper crash/resume gate, mock profile | `342fdfcb…` | PASS |

Reproduction commands:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh --non-production-mock-source
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  --non-production-real-pyth-lab \
  real_pyth_router_verifies_then_post_update_and_clutch_append_are_atomic
programs/clutch-sbf/scripts/run_keeper_gate.sh
```

The keeper gate executed 24 permissionless actions and one owner-signed action.
It deliberately killed the first keeper after four actions during incomplete
ClearWork, resumed from chain state with a fresh keeper, and reached the same
fail-closed `Blocked` state across a further fresh restart. It never attempted
`CloseGeneralEpoch`. Safe leaves and their ledgers were absent; the CLEARED
Epoch, Window, and epoch funding ledger remained as replay anchors. Exact value
conservation closed at `cash 32 + locked 17 = endowed 49`, with Eggs `[17,17]`
and custody `49`.

In the real-Pyth laboratory campaign, the real router first persists a Verified
locally signed 13-of-19 synthetic VAA. In a later transaction, the captured
real receiver's `PostUpdate` and Clutch `AppendSourceArchiveV2` are adjacent and
atomic. Missing adjacency refuses with the archive unchanged; wrong Config or
feed rolls back both the receiver-created update and archive. Commit `5ab10b0`
then seals the one-record archive and resolves a categorical market to payout
cell 1 because the entire admitted conservative interval
`[99,980,929, 100,019,071]` lies in that cell. This does not establish a current
network price, provider availability, the upgraded 3-of-5 trust substrate,
redemption, a multi-boundary shared window, or production source admission.

## Current persistent deployment rent

With exact `max_len = 2,082,320`, loader-v3 persistent account data is:

- ProgramData: `45 + max_len = 2,082,365` bytes;
- Program: `36` bytes.

At the audited rent schedule:

| account | rent-exempt minimum |
| --- | ---: |
| ProgramData | `14.494151280 SOL` |
| Program | `0.001141440 SOL` |
| total persistent rent | **`14.495292720 SOL`** |
| transient deployment Buffer | `14.494095600 SOL` |

The Buffer is separate deployment liquidity and is normally recyclable. The
persistent total excludes transaction fees and assumes exact-size allocation,
leaving no upgrade headroom. A ten-SOL persistent-rent target requires an ELF
no larger than `1,436,444` bytes: another `645,876` bytes, or `31.02%`, below
this artifact. Recalculate from the final production-source ELF and chosen
`max_len` before any funded deployment.

## Historical first pass

The earlier clean closure at `ba580c64ca2b2125771391d7184afc3f67ce8227`
produced a byte-identical `2,160,072`-byte default ELF with SHA-256
`a6381fbe211e400788615e1c588938266bed14bc8f0fc12babf76350bc24cbe2` and
source-closure digest
`2012201b8937fec50afd08a1e075a276d965ced56c3922df5e47b1c33e122438`.
Its persistent loader rent was `15.03644664 SOL`. Those values remain useful as
the pre-repair comparison point; they are not the current runtime artifact.
