# Current unsealed SBF snapshot — 2026-08-22

Status: **local unsealed engineering evidence only**. The declared SBF input
closure was clean and committed at `ba580c6`; other review and measurement files
were still in flight outside that closure. This is not a release, deployment
candidate, signed manifest, or authorization to fund or deploy.

Two fresh offline default-profile builds from separate temporary target
directories produced byte-identical stripped ELFs:

| fact | value |
| --- | --- |
| ELF bytes | `2,160,072` |
| ELF SHA-256 | `a6381fbe211e400788615e1c588938266bed14bc8f0fc12babf76350bc24cbe2` |
| tracked source-closure files | `129` |
| source-closure SHA-256 | `2012201b8937fec50afd08a1e075a276d965ced56c3922df5e47b1c33e122438` |
| source commit | `ba580c64ca2b2125771391d7184afc3f67ce8227` |
| profile | default; empty production source registry |

The full offline artifact audit rebuilt from fresh target directories twice and
then once more with a relocated Cargo home. All three stripped ELFs were
byte-identical. The source-closure digest is the audit's path-ordered,
per-file-digest fold over the 129 tracked build inputs. This makes the result
reproducible from the named commit; it does not turn the bytes into a seal or
release.

## Stack classification

The pinned backend emitted 37 diagnostics for 28 dependency symbols while
compiling intermediate objects. The current final unstripped ELF was then
checked using the repository audit's two independent rules:

- diagnosed symbols surviving final LTO: `0`;
- final text function symbols: `1,083`;
- final text function addresses: `1,080`, all disassembled;
- direct `r10` stack references inspected: `66,633`;
- deepest direct `r10` offset: `4,096` bytes;
- out-of-frame direct `r10` references: `0`.

The intermediate diagnostics therefore do not identify a resident diagnosed
function in this ELF. The same audit verified 88 registry archives, 12
first-party packages, one vendored package, the exact ten-symbol dynamic syscall
surface, ELF segment/entry shape, and loader sizing. It is still not the bank,
manifest, portable second-host, signature, deployment, or independent security
review required for a release.

## Current persistent deploy rent

With exact `max_len = 2,160,072`, loader-v3 persistent account data is:

- ProgramData: `45 + max_len = 2,160,117` bytes;
- Program: `36` bytes.

The local devnet clone reports:

| account | rent-exempt minimum |
| --- | ---: |
| ProgramData | `15.03530520 SOL` |
| Program | `0.00114144 SOL` |
| total persistent rent | **`15.03644664 SOL`** |

This excludes transaction fees and any transient deployment-buffer liquidity.
It also assumes exact-size allocation, leaving no upgrade headroom. Ten devnet
SOL is therefore at least `5.03644664 SOL` short even before fees, regardless of
the separate payer-address mistake. Recalculate from the final production-source
ELF and chosen `max_len` before requesting faucet funds.
