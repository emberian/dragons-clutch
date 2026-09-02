# The General accelerator is deployed on devnet — 2026-09-02

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized. This records one ordinary
single-program deploy under the standing devnet grant in `AGENTS.md`.

Tree root `/Users/ember/dev/dclutch`. Source commit
`324528a4b8f1e1d1071e383d3f9dbd07ed4ea884`, built from two independent detached
worktrees at that commit — never from the shared dirty tree.

## Why this deploy is not a cohort deploy

The standing grant's condition (a) — *full redeploy only, every program in the
cohort* — governs **cohort role sets**. The General accelerator is not one of
the seven sealed cohort roles (registry, rent, custody, resolution, claims,
trading, core); it is an accelerator, admitted through an
`ExecutionStrategyCertificateV2` rather than through the infrastructure
profile, and no cohort's release set names it. Deploying it therefore disturbs
no cohort's identity, and abandoning a cohort to redeploy it would be the
larger act, not the smaller one. Conditions (b) and (c) still bind and are met:
(c) the deploy is from a named commit, and (b) the load simulator's population
life belongs to the cohort lane whose cohort this accelerator will serve —
cohort-14 — and is named in the runbook step below rather than claimed here.

## The build

`cargo build-sbf --manifest-path programs/dclutch-general-accelerator-sbf/Cargo.toml -- --locked`,
the ordinary release invocation: no `hot-cu-profile`, no diagnostic feature.
Two independent worktrees at `324528a4`, two independent `CARGO_TARGET_DIR`s:

| build | bytes | SHA-256 |
| --- | ---: | --- |
| A | 302,256 | `61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae` |
| B | 302,256 | `61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae` |

**A == B, byte-identical.**

## The deployment

| fact | value |
| --- | --- |
| cluster | devnet (`EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`) |
| program id | `8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY` |
| ProgramData | `HcxFzWKaFzrVVnvgx6BWuNbo278pgpYY5CrxyVe67Sxb` |
| loader | `BPFLoaderUpgradeab1e11111111111111111111111` |
| deploy signature | `3TtiaVkrubvTjhMu4GTD1369AwGYiksSG5WRBn6Sz3SbBB8SY4SZcvMbTay6pcGKVweE766BCxu7sGt3e4aYKnPR` |
| deployment slot | **491,959,038** |
| upgrade policy | `ExactAuthority` |
| upgrade authority | `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` (the deployer) |
| ELF digest | `61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae` |
| ELF bytes | 302,256 |

The deployment slot and the authority are not transcribed from the CLI's
summary line: they are hostile-decoded out of the finalized ProgramData account
image by the same parse the on-chain authenticator runs. The 45-byte header
reads

```
03000000 feb2521d00000000 01 3b65a93a665346993e31fd6ed5277a9814c37f43076c363372c8a1041df37ade
```

— enum tag 3 (`ProgramData`), slot `0x1d52b2fe` = 491,959,038, authority
present, authority bytes = `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`.
The account is 302,301 bytes = 45 + 302,256, so the ELF tail carries **zero**
padding and its SHA-256 is the built ELF's digest unchanged. That equality is
what lets `dclutch_shadow_accelerator_auth_v4::deployment::authenticate_current_deployment`
— which hashes `ProgramDataV3View::elf()` — accept a release whose `elf_digest`
is the build's own.

### Read-back

`solana program dump` of the live program, compared byte-for-byte against build
A: **IDENTICAL**, 302,256 on-chain bytes against 302,256 built bytes, with no
nonzero tail.

## Cost, measured

| item | lamports | SOL |
| --- | ---: | ---: |
| ProgramData rent | 1,915,282,857 | 1.915282857 |
| Program account rent (36 bytes) | 1,038,612 | 0.001038612 |
| **rent subtotal** | **1,916,321,469** | **1.916321469** |
| transaction fees | 1,515,000 | 0.001515000 |
| **total spent** | **1,917,836,469** | **1.917836469** |

Deployer balance 34.391688319 SOL before, 32.473851850 after. **The whole cost
is one program's rent**; 0.0015 SOL of fees is what was spent beyond it, against
a stated ceiling of 2 SOL.

Two rent facts worth keeping. `solana program deploy` at CLI 4.0.2 allocated
`Data Length` **exactly** the ELF length — not the historical `2 × len` — so the
ProgramData account is 45 + 302,256 and the program is not growable in place; a
larger successor ELF needs `--max-len` at deploy time or a fresh identity.
And the affine devnet model `890,880 + 6,960·n` predicts 2,104,905,840 lamports
for 302,301 bytes against 1,915,282,857 observed: it **over-predicts by ~9%**,
the same direction cohort-9 measured. Keep using it as a ceiling, never as a
quote.

## Provenance of the evidence

Job directory `~/jobs/dclutch-general-devnet-20260902` (mode 700):
`build-general.sh`, `deploy-general.sh`, `elf/`, `elf-B/`, `deployed/`,
`logs/`, `keys/`. The program keypair is `keys/general-accelerator.json`; the
deployer is the standing devnet deployer and no other key signed.
