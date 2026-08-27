# Checked release candidate — 2026-08-26

This records the first end-to-end run of the complete checked-release pipeline
over the seven real dClutch role programs and the three accelerator programs,
built from one exact source commit.

**What this is.** A LOCAL, reproducible release candidate at evidence-ladder
level: an offline evidence chain binding one source revision to ten exact SBF
artifacts, their constructed Loader V3 accounts, one five-role execution release
set, and one immutable Core/Registry/Rent infrastructure join.

**What this is not.** It is not a deployment. It is not devnet or mainnet
evidence. It is not local-validator execution evidence — nothing was launched,
signed, submitted, funded, or published. It is not an official release, and no
frontend or address may be described as official on the strength of it.

**It is already superseded in part.** The Trading artifact is expected to change
when the active heap lane lands; the General accelerator artifact changed twice
during this session's own runs. **The moment either ELF changes, this candidate
is superseded** and must be regenerated. That regeneration costs about one
minute — see *Re-run cost* below.

> ### Regenerated 2026-08-27 at `35075a34`, and the pipeline is still green
>
> Every digest below is from `ec557e81`, **248 commits back**, with nine of the
> ten role trees changed since. They are dead and this document keeps them
> because they are what the run they describe produced, not because anything
> should still match them.
>
> The verdict from the regeneration, which is what is load-bearing:
>
> - Every verification in *Verification results* below ran and passed again —
>   ten `create`/`verify`/`inspect` triples, the five-role set, the
>   infrastructure join, and every byte-identical text-projection `cmp`.
> - **`sbf_build_diagnostics_total=0`, down from 36**, and
>   `sbf_build_diagnostics_accepted=false`. Finding 1 below is CLOSED: the run
>   no longer needs `--allow-build-diagnostics` and did not use it. The
>   dealer-accelerator's overflowing `hot_v3::process_hot_execution_v3`
>   monomorphization is gone, and so is the 65-diagnostic
>   `relay_transport_v1::process_relay_transport_v1` frame that JRNY-1 found
>   independently on the Resolution artifact. **All ten roles are at zero.**
> - Toolchain pins unmoved: solana-cli 4.0.2, cargo-build-sbf 4.0.0, platform
>   tools v1.53, SBF rustc 1.89.0.
> - Summary at `/private/tmp/opsfinish/release-candidate/SUMMARY.txt`,
>   `source_digest=0603d72f7e58838d…`.
>
> Two structural notes for whoever regenerates next. `dclutch-series-shadow-sbf`
> was workspace-*excluded* with its own `Cargo.lock` at `ec557e81` and is a
> plain member with none today, so `root_cargo_lock_digest` differs from this
> document on that basis alone and `target_dir_for`'s excluded-crate branch is
> now dead code. And `--keep-elf` re-stamps a STALE diagnostics total, because
> the line that resets `build-diagnostics.txt` sits inside the build guard.

## Reproducing it

```sh
tools/release/checked-release-candidate.sh \
  --work /private/tmp/dclutch-release-candidate \
  --commit ec557e81550ac1664fe3ad341d81cce9f9494b4f \
  --allow-build-diagnostics
```

The script archives the commit into a scratch tree (never the shared checkout's
`target/`), builds every program with `cargo build-sbf`, constructs the Loader
accounts and canonical metadata, and then runs construction *and* verification
*and* inspection for each manifest, comparing all three text projections. It is
idempotent: re-running it rebuilt every artifact and manifest byte-for-byte
identically.

`--commit` selects the *program sources*; the script itself is whichever copy
is in your checkout. The runs behind this document used both the version at
`ec557e81` and the later fix in finding 4, and both produced identical
summaries at this commit.

## Source and toolchain

| Fact | Value |
|---|---|
| Source revision | `ec557e81550ac1664fe3ad341d81cce9f9494b4f` |
| Source digest | `a07b4c30f50067db8e54381439468dad9a9267ecc665ab443a087255b0012c4b` |
| Root `Cargo.lock` digest | `efedad18ba64ef5b58cf11290e1bc6001e890884470e82d62d9b57099c614d32` |
| Build rustc | `rustc 1.89.0 (solana platform-tools v1.53)` |
| Solana | `solana-cli 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)` |
| Builder | `cargo-build-sbf 4.0.0` |
| Target triple | `sbpf-solana-solana` |
| ELF machine | `EM_SBF` (263) for all ten artifacts |

The source digest is SHA-256 of `git ls-tree -r --full-tree <commit>`: every
tracked path, mode, and blob identity, independent of checkout state or file
times. `dclutch-series-shadow-sbf` is excluded from the root workspace and
resolves against its own `Cargo.lock`; its manifest records that lock's digest,
not the root's.

`rust-toolchain.toml` pins the *host* toolchain to 1.97.1. The SBF artifacts are
built by platform-tools' own rustc 1.89.0, which is what the manifests record.

## Per-artifact hashes

| Role | ELF SHA-256 | bytes | checked-release manifest SHA-256 |
|---|---|---|---|
| core | `d5f9c1834eca97d3243a15a6b991cf11952ca716bfb70b5de6641509247b4fb8` | 1,005,408 | `1b73c0e4325b8b0464c62638d4f8a231e724dd20de24df196dfa4971a0313314` |
| claims | `8c66e8efcb00939d04e964e914d85e37d4e2c3bb1c900b8b2e55188810c87f6c` | 1,074,256 | `b61657e14d85cccbede9a06325529557b749558ab15a8777e5e24794fcd48a2f` |
| trading | `c853839ee2f9d1cb435a67ed9d9768f34ea02e519c1a419558ddb77eba47a87d` | 1,287,728 | `0f86ea808919f85d1a99af96a46ed033a8272e85c378cf91a0dd42e0d3744900` |
| resolution | `9722b3241d252c1a0d51bf5eef178aa6aace963eaa0865da9e89cce54f8fcb8c` | 463,576 | `d1ace5e6c9ca835b9c1d3ee002a94a9118a303d1da46cb2514029ff5b0251b00` |
| custody | `a0f631e62806551330658eae70cf45de9e6521bde144d78591d287ea687b4853` | 330,464 | `3b741390277971cb23608591316ab771bc30fb730292e3ffddb383c2262cb756` |
| registry | `d5fed2a63c73047a89fc5b04543ba641e5d9faa04ee720afa6a5826b6ac0808f` | 225,648 | `e180e6677266b043f101b03acee66c13cb233f41a0be61a21d927d4a8998f12a` |
| rent | `bc170acbac10f06353f928be10a5b23ef8141b0a8349ae9b92acaf3d99ed126d` | 152,352 | `83ab39c52879f19d5e1bd49d7a5f365dfaad3decd2a2b63b88f4106497fd1bda` |
| general-accelerator | `46ad714bd475e046fa80a0a0c2ba3b154aae190ff1ee3e841e24261856aee343` | 193,968 | `9f68e4effbfae0aa70e3991ad36ad3b2cc159fa7e3f44d6bfea2301165cc377d` |
| dealer-accelerator | `851c71c5eee9a6837118023c1483c808c17c4ebc0b745eb9f4001bd991c51d00` | 599,360 | `24663e09765b006d0f4b0d35e3bf85e7196925437c398007b55bfd6c2a8ae9d0` |
| series-shadow | `1f3f826f9ab6b448a84eecb192c17b7c167da562e35d9fa6908a2e4eb8353e55` | 110,768 | `6dbe442e4202f730537ff6040395c3c999ca2118c2d0dbf2fb7167daa3ee650c` |

Roles are `dclutch-core-sbf`, `dclutch-claims-sbf`, `dclutch-trading-sbf`,
`dclutch-resolution-proof-sbf`, `dclutch-custody-sbf`, `dclutch-registry-sbf`,
and `dclutch-rent-sbf`. `dclutch-series-shadow-sbf` builds cleanly as a
standalone `cargo build-sbf` invocation, confirming that its root-workspace
exclusion is a feature-unification problem, not a build problem.

## Manifest digests

| Manifest | SHA-256 |
|---|---|
| `ExecutionReleaseSetV1` preimage (336 B) | `b166713e52f0f0abe41934eaa2f689b6b44f23191467af818123b1bddfef0a5d` |
| `CheckedExecutionReleaseSetV1` (1,592 B) | `9579c58de52b358d7068d5f64583702ba2c0b8a28240768eac34fb09162507fc` |
| `ProtocolInfrastructureProfileV1` (144 B) | `2633f3906de1134377219bcde7fc07f31b427f0852fed5bf154627a7f10aed04` |
| `CheckedInfrastructureV1` | `78021796355229ebdde22043817583d9396ae47b0bf647da0d867b9e6bd58a75` |
| Infrastructure profile PDA under Core | `ef926a3f452ed381c1f9b37e5c4b636788f6468661699f4e74fb414c4576f68d` |

## Verification results

Every check the release tool performs passed. Construction, verification, and
inspection each produced byte-identical text projections:

- ten `create` + `verify` + `inspect` passes over `CheckedReleaseV1`;
- `derive-set`, `create-set`, `verify-set`, `inspect-set` over the five-role
  `CheckedExecutionReleaseSetV1`;
- `derive-infrastructure-profile`, `create-infrastructure`,
  `verify-infrastructure`, `inspect-infrastructure` over
  `CheckedInfrastructureV1`, including the immutability requirement on Core,
  Registry, and Rent and the profile-PDA derivation under Core.

Reproducibility was tested, not asserted. Four independent runs at this commit —
two cold, one warm, one evidence-only — produced byte-identical summaries. A run
into a *different* scratch root, i.e. a different absolute build path, produced
identical ELF and manifest digests too, so these artifacts are not
path-dependent.

## Honest boundaries

Four boundaries are carried inside the manifests themselves as `assumption=`
lines, so they travel with the evidence rather than living only here.

**Loader accounts are constructed, not observed.** No program is deployed
anywhere, so there is no account to snapshot. The Program and ProgramData bytes
were built from each ELF by `dclutch-release-tool loader-accounts`, whose text
projection is labeled `evidence_class=predicted-loader-state-not-observed`.
Verifying constructed accounts against the ELF they were constructed from proves
layout, not deployment. `deployment_slot=0` is a constructed genesis-install
value, not an observed slot.

**Program addresses are candidate-local.** Each is SHA-256 of
`dclutch/checked-release-candidate/program-id/v1\nrole=<role>\n`. They are
stable across rebuilds and across an artifact changing under a role. No private
key exists for any of them, none is registered anywhere, and none names a
deployed program.

**The semantic release identity has no owner.** Every role, Registry, and Rent
persists a `semantic_release_id` inside its `ArtifactReleaseV1`, but no
first-party contract in this tree owns or decodes a role-program release
preimage. These manifests therefore carry `semantic_kind=unowned` over a
candidate-declared preimage. Naming a real owner is an open protocol
obligation, not something host tooling should settle.

**One artifact carries a toolchain warning of undefined behavior.** See below.

## Findings

### 1. The dealer accelerator's Trading monomorphization overflows its stack frame

`cargo build-sbf --manifest-path programs/dclutch-dealer-accelerator-sbf/Cargo.toml`
emits 36 copies of:

```text
Error: A function call in method _ZN19dclutch_trading_sbf6hot_v324process_hot_execution_v3...
overwrites values in the frame. Please, decrease stack usage or remove parameters
from the call. The function call may cause undefined behavior during execution.
```

Attribution is exact: building `dclutch-trading-sbf` itself emits **zero**, as
do `dclutch-general-accelerator-sbf` and `dclutch-series-shadow-sbf`. Only the
feature set the dealer accelerator requests of `dclutch-trading-sbf` produces
the overflowing monomorphization of `hot_v3::process_hot_execution_v3`.

`cargo build-sbf` exits zero for this, and nothing downstream can see it: the
ELF is well-formed, `EM_SBF`, and every release-tool check passes on it. The
runner therefore counts the diagnostics per role and **refuses by default**;
this candidate was produced with `--allow-build-diagnostics`, which stamps
`sbf_build_diagnostics_total=36` and
`sbf_build_diagnostics.dealer-accelerator=36` into the summary.

Owner: the W2 hot-fast-path lane (`hot_v3.rs`, its file). This is a finding to
fix there, not to relabel here.

**CLOSED, measured 2026-08-27 at `35075a34`: the total is 0 and every one of the
ten roles is 0.** The regeneration needed no `--allow-build-diagnostics` and did
not pass it, so the runner's default refusal is now a gate that a real run
clears rather than one every run has to be waved past.

### 2. The capability-execution bundle path cannot yet be exercised on real accelerators

`create-capability-execution` works, and all three accelerator ELFs have
complete, immutable `CheckedReleaseV1` manifests ready for it. What is missing
is upstream: **no non-test producer of `ExecutionStrategyCertificateV2` exists
in the tree.** The only construction sites are `#[cfg(test)]` fixtures
(`crates/dclutch-general-adapter-contract/src/artifacts_v3.rs`, the contract's
own tests). Producing a bundle would mean minting a certificate, strategy, and
`CapabilityProgramV4` with fabricated capability semantics, which is authoring
capability records in host tooling — the wrong owner.

There is a real ordering constraint behind this worth writing down.
`CheckedCapabilityExecutionV1::validate` requires the certificate to name the
accelerator's `ArtifactReleaseIdV1`, which is a function of the accelerator's
program address and ELF digest. **A capability manifest therefore cannot be
finalized before its accelerator's deployment address and artifact are fixed.**
Capability authoring and accelerator release are not independent steps.

Owner: the capability/family lanes (General, Dealer, Series).

### 3. Frontend: two rules in the same app contradict each other on Core vs Registry

This is the specific thing blocking an honest un-gate, so it is written up in
full in the next section.

### 4. Excluded packages cannot share a target directory with the workspace

`dclutch-series-shadow-sbf` is excluded from the root workspace and resolves
against its own `Cargo.lock`. When its build shared one `CARGO_TARGET_DIR` with
the root-workspace builds, the first cold run succeeded but every warm rebuild
then failed with `one version of crate dclutch_execution_strategy_contract used
here, as a dependency of crate dclutch_trading_sbf` — two units of one path
dependency in the same graph. Its isolated build is clean, and the artifact it
produces is byte-identical to the one recorded above
(`1f3f826f9ab6b448a84eecb192c17b7c167da562e35d9fa6908a2e4eb8353e55`), so this
was a build-harness hazard rather than a defect in the program.

The runner now parses the root `members` list and gives any non-member package
its own target directory. Anything else driving builds across this boundary —
including a future un-exclusion of `dclutch-general-sbf` — needs the same care.

### 5. Translation-validation evidence was not produced

`create-translation` requires the 21 inputs from
`tools/direct-translation-validator/check.sh`, which runs `lake build` over
`formal/dclutch-semantics`. That is a separate, heavy lane and was not run. The
path is wired and untested by this candidate.

## The wallet un-gate contract (specification for the frontend lane)

**Not implemented here.** `apps/dclutch-web` was read only. This section
specifies what the frontend would need in order to honestly stop gating wallet
signing against a *local validator running these exact ELFs*.

### The blocker: Core is not the Registry program

`apps/dclutch-web` currently holds two incompatible rules:

- `lib/infrastructure.ts` (~line 282) refuses when
  `execution.artifacts.core.program === registryArtifact.program` — Core **must
  differ** from Registry. This agrees with the protocol contracts
  (`CheckedInfrastructureV1::validate` refuses aliasing across Core/Registry/Rent)
  and with the local bootstrap (`validate_program_ids` requires all seven to be
  pairwise distinct).
- `lib/releaseRegistry.ts`, in `prepareRegistryActivation`, throws unless
  `evidence.releaseSet.roles.core.program === input.registryProgram` — Core
  **must equal** Registry. `parseCache` repeats the conflation
  (`artifacts[0].program !== registryProgram` throws), and
  `lib/releaseRegistry.test.ts` bakes it into its fixture
  (`expect(decoded.releaseSet.roles.core.program).toBe(fixture.registry)`).

`releaseRegistry.ts` is the one that is wrong. The Registry program is the
program that owns the record and activation-cache PDAs and executes the
`DCLTRIX1` instruction; the Core role's program is a different program in the
release set. `programs/dclutch-registry-sbf` derives the activation cache under
its own program id, and the bootstrap passes `--registry-program-id` and
`--core-program-id` as separate values. Any honest seven-program release set —
including this candidate — is refused today by that check.

**Contract item 1:** `prepareRegistryActivation` must take the Registry program
as an input independent of the release set's Core binding, and must stop
requiring them to be equal. `parseCache` must drop the same requirement. The
test fixture must be rebuilt with Core distinct from Registry, because the
current fixture cannot distinguish the bug from correct behavior.

### What un-gating additionally requires

`prepareRegistryActivation` is otherwise already a hard, byte-exact gate, and it
should stay that way. Against a local validator it will still demand, at
finalized commitment:

**Contract item 2 — the manifests.** Base64 of `multiprogram.checked` (1,592
bytes) plus all five `<role>.checked` manifests. This candidate's are at
`$WORK/set/multiprogram.checked` and `$WORK/evidence/<role>/checked.bin`.

**Contract item 3 — finalized Registry records.** For the release set and for
each of the five artifact records, a finalized account at the derived PDA whose
data is byte-identical to the evidence, owned by the Registry program, rent
reserved, with a vacant staging cursor. Producing these is the local bootstrap's
`prepare` step, which is W1's surface.

**Contract item 4 — Loader state matching the checked manifests exactly.**
`authenticateDeployment` requires, per role: `Program` owned by the Upgradeable
Loader, executable, exactly 36 bytes, variant 2, linking the canonical
ProgramData PDA; `ProgramData` owned by the loader, non-executable, variant 3;
the current ELF digest equal to the artifact's; and — because the checked
release is supplied — the account *geometry and digests* equal to
`program_account_sha256` / `programdata_account_sha256` / lengths in the
manifest.

This is the load-bearing consequence: **the constructed account bytes in this
candidate must equal the bytes `solana-test-validator --upgradeable-program
ADDRESS ELF none` actually creates at genesis.** They are constructed to be —
same 36-byte Program record, same 45-byte ProgramData boundary, same `None`
authority, `deployment_slot=0` — but that equality is a *prediction and remains
untested until a validator is run*. Confirming it is the cheapest next step on
this path, and it is a pure diff: launch the validator with these ELFs at these
addresses, read the two accounts back, and compare against
`$WORK/evidence/<role>/program-account.bin` and `programdata-account.bin`.
If genesis differs (a nonzero slot, a different authority encoding), the
candidate's manifests must be rebuilt from the *observed* accounts and the
`assumption=` lines updated to say so — at which point the evidence stops being
a prediction and becomes local-validator execution evidence, one full rung up
the ladder.

**Contract item 5 — addresses.** For a local run, use these candidate-local
addresses so the manifests match. They are not deployed anywhere.

| Role | program | ProgramData |
|---|---|---|
| core | `BKWABnJck4Mr6ABg3wxF3H9oAjsRWDKEowMAfF3GXxTy` | `8STtE7jTRJiDW4Uhehodhy4A6rydygMNnosLhe2cVcd9` |
| claims | `3vUZpLtmQpqcyq6ja4BPsNQrQGxnwaM4yGFkSPebBMSM` | `9WyviXwLwnWd44nbC4Abh6fG7gKPGQwUU7uo9E42kuq7` |
| trading | `EzNsMh6nZEX7py7zG8q7bF4tmAwsDdepz9kEmXcH4H19` | `8k2qykZBNZfLbjKnpcgvt4MMLSuKCB19a5hvzgQhbc12` |
| resolution | `86TK6juVMgPrb2ohtnbqZhNzjMhB62h7XLdY9McbssJK` | `3RJez2GYZeThAcZKGUHQyYK6zqfYokSMEg4TepUyabW5` |
| custody | `E73L5FK1s1BfPr5eUrRSrkRp29zLkJTXdpQUkaoSMLiQ` | `3DscA9HmixuqCP965SQX3NtwxmwbDrPnaeAU23xsg3M4` |
| registry | `36bGDZCNG56HghmP34MdMCfNEFBHqR5eLBPzpPh8UjJz` | `2zBM6ErGVPSekUpY9ixsfhMc8ti5XQY2Jy9Z33Jbn6u9` |
| rent | `5aMSnNYs9VKUBoDKy2Hzbw7o6RoEL35vdnZVYzakU2P3` | `ALriFzyzQQ1xYjiiA3sNxg8fn1Wu9K6b1XhU6SGvid5t` |
| general-accelerator | `34wGgC3WkQPwJ9asNqEryeQsG6kNdofGs557bG24Kism` | `2hRdBcFSmw2mVNuYUGmrCXu4BGkGtWCinMEPurrAdkz1` |
| dealer-accelerator | `Fg2djBTcdmf51wg1hFb9NXXEK2AhuSa6pXw9jPG5StHM` | `7EfSgc1z25Stf6FL6CPf4CHAgomeEjjxPVGD8wmDNxCQ` |
| series-shadow | `2a2gGTQGfpccStyT8MfZ1FH9XaPW2GoYjZkSsAob5k8b` | `7iCqKax8spDc8Bs8dQHE29sFuPiTCitqS4dmNLFgeTck` |

### What un-gating does NOT license

Signing is already reachable in `DirectTradeWorkspace` and
`RationalRepresentationWorkspace`; the app signs and exports for an external
submitter and never auto-submits. That property should survive un-gating.

A green `prepareRegistryActivation` against a local validator authorizes exactly
one sentence: *this browser observed a local validator whose finalized Registry
records and Loader accounts match a named checked release set built from a named
commit.* It does not make the addresses official, does not make the frontend
official, and does not transfer to devnet or mainnet. Per `AGENTS.md`, no
deployment or frontend is official without a checked release manifest — and a
manifest over candidate-local addresses and constructed accounts is not that
manifest.

## Re-run cost

Measured on this machine:

| Scenario | Wall time |
|---|---|
| Cold: empty work dir, all 10 SBF builds, release-tool build, full evidence chain | **79 s** |
| Warm: fresh archive, incremental SBF rebuild, full evidence chain | **28 s** |
| Evidence chain only (`--keep-elf`) | **2 s** |

When the W2b heap lane lands and the Trading ELF changes, regenerating this
candidate is one command and about a minute. Nothing in this document should be
preserved past that point: **replace it, do not amend it.**

## Artifacts on disk

Under `--work` (default `/private/tmp/dclutch-release-candidate/`, not tracked):

```text
SUMMARY.txt                          every hash and identity in this document
build-diagnostics.txt                per-role SBF diagnostic counts
build.log, build-<role>.log          complete build output
elf/<role>.so                        the exact artifacts
evidence/<role>/semantic.bin         candidate-declared unowned preimage
evidence/<role>/metadata.txt         canonical Metadata V1
evidence/<role>/program-account.bin  constructed Loader V3 Program
evidence/<role>/programdata-account.bin  constructed Loader V3 ProgramData
evidence/<role>/checked.bin/.txt     CheckedReleaseV1 and its projection
set/execution-release-set.bin        336-byte Registry preimage
set/multiprogram.checked/.txt        CheckedExecutionReleaseSetV1
infrastructure/profile.bin           144-byte Core-owned profile
infrastructure/infrastructure.checked/.txt   CheckedInfrastructureV1
```
