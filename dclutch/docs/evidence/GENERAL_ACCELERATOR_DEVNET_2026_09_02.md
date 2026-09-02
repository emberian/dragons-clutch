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

---

# The cohort-14 runbook step

This is written for the cohort lane and stops before every act it does not
authorize. It assumes cohort-14 exists: seven roles deployed from a commit that
contains `a517d27c` (inline input transport), `90a8563f` (the Registry observes a
deployment at finalization) and `271ce0ed` (the hot path authenticates by slot
pin). Cohort-12 and cohort-13 contain **none** of the three, which is why the
step below cannot be run against either and why the on-chain half of this lane's
proof is cohort-14's and not this lane's.

## 0. What cohort-14 owes before step 1 — the one piece of code this lane did not write

The accelerator's `ArtifactRelease` record must be **published and finalized in
cohort-14's Registry**. Under `90a8563f` that finalization *is* the observation:
`build_record_publication_step_v1` sees `ARTIFACT_RELEASE_SCHEMA_ID_V1` and
derives the Program and ProgramData metas from the record's own content, and
`observe_artifact_release_deployment_v1` compares them before the staging cursor
closes.

The record body is emitted on **stderr** by step 2 below
(`artifact_release_body`, 216 hex-encoded bytes), so it can be published with the
existing `runtime::publish_record` and nothing new. The architecturally correct
home, though, is `prepare`: the accelerator's release belongs beside the seven
roles' in the cohort's infrastructure publication, minted by the same
`plan::release_facts`, and that means one more optional role-shaped flag group
(`--general-accelerator-program-id`, `--general-accelerator-elf`,
`--general-accelerator-sha256`, `--general-accelerator-semantic-release-id`,
`--general-accelerator-observed-programdata`, `--general-accelerator-live-elf-sha256`,
`--general-accelerator-expected-upgrade-authority`). **This lane did not add it**:
`plan.rs`'s `prepare` surface is the cohort lane's and was mid-founding.

Two facts about the accelerator that cohort-14 must state, because nothing
derives them:

- **It has no protocol-owned semantic release identity.**
  `upgrade::checked_semantic_release_preimage_v1` refuses any role outside the
  seven with *"role has no protocol-owned semantic release identity"*, so
  `--general-accelerator-semantic-release-id` is an operator-stated hex. The fix
  is a `SourceSemanticRoleV1::GeneralAccelerator` label, after which the id is
  the ordinary artifact-derived digest and the flag becomes a check rather than
  an input.
- **It is `ExactAuthority`, not `Immutable`.** The deployer retains the upgrade
  authority, so `--general-accelerator-upgrade-authority` takes
  `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` and never the literal
  `immutable`. Getting this wrong mints a record that says the accelerator can
  never be redeployed; the substitution refuses at finalization by name, and
  `record_v1.rs`'s `devnet_general_accelerator_observation` pins that.

## 1. The four evidence files

Three identities have no author in this tree and one has, so all four are files
whose bytes a reader can re-hash. Put them in the cohort job directory.

```
$JOB/general/selection-policy.txt       first line: dclutch-general-selection-policy-v1
$JOB/general/compiler-release.txt       first line: dclutch-general-compiler-release-v1
$JOB/general/toolchain.txt              first line: dclutch-general-toolchain-v1
$JOB/general/translation-validation.bin 688 bytes, a canonical CheckedTranslationValidationV1
```

The first three are `sha256` of the exact file — verify with `shasum -a 256`.
The fourth has **no hex path**: it is decoded by
`dclutch_release_tool::CheckedTranslationValidationV1` and its identity comes
from that type's own `translation_validation_id()`. Produce it with
`dclutch-release-tool create-translation` over an evidence directory from
`tools/direct-translation-validator/check.sh`.

**Say what the toolchain and compiler-release files contain.** The exact
`rustc -Vv`, `cargo-build-sbf --version`, `solana --version`, target triple and
build command for the ELF below belong in the toolchain file; the source
revision, the package source digest and the `Cargo.lock` digest belong in the
compiler-release file. Nothing yet checks that they describe *this* ELF — that
is the honest remaining gap and it is named again at the bottom.

## 2. Compile the General market

```
dclutch-local-successor-bootstrap devnet-general-market \
  --registry-program-id $COHORT14_REGISTRY \
  --plan $JOB/plan.json --rpc-url "$URL" --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --general-accelerator-program-id 8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY \
  --general-accelerator-elf /Users/ember/jobs/dclutch-general-devnet-20260902/elf/general-accelerator.so \
  --general-accelerator-semantic-release-id $SEMANTIC_RELEASE_HEX \
  --general-accelerator-upgrade-authority 4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP \
  --general-policy $JOB/general/policy.json \
  --general-selection-policy $JOB/general/selection-policy.txt \
  --general-compiler-release $JOB/general/compiler-release.txt \
  --general-toolchain $JOB/general/toolchain.txt \
  --general-translation-validation $JOB/general/translation-validation.bin \
  --general-quote-surplus-beneficiary $CAMPAIGN_PAYER \
  --price-update $JOB/general/price-update.bin --window-start "$(date +%s)" \
  --band-anchor 15000 --band-volatility-bps 200 --band-window-slots 10000 \
  --band-plausible-half-widths 3 --band-max-cell-share-bps 9000 \
  > $JOB/general/market.json 2> $JOB/general/accelerator-observation.txt
```

`market.json` is the campaign's input; `accelerator-observation.txt` carries the
deployment slot, the ELF digest, the artifact release id and the 216-byte record
body step 0 publishes. **Read it before founding**: if
`deployment_slot` is not 491,959,038 the accelerator has been redeployed and
every certificate this market compiles pins the wrong artifact.

`$JOB/general/policy.json`, whose fields are refused individually if unknown,
defaulted or noncanonical:

```json
{"schema":"dclutch-general-devnet-policy-v1",
 "windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,
            "max_orders_per_candidate":32,"max_pages_per_candidate":32,
            "continuation_reward_lamports":1},
 "external_widths":{"linked_basis_prefix":256,"result_domain":192,"rent_sysvar":17,
                    "core_market":320,"activation_cache":160,"upgradeable_program":36,
                    "trading_programdata_prefix":45,"claims_programdata_prefix":45,
                    "core_programdata_prefix":45,"realm_record":112,"rent_credit":48},
 "token_account_bytes":165}
```

Those are the widths the executed accelerator campaign ran against. **They are
an input and not a measurement**: reconciling them against cohort-14's own live
account widths is the General-hot follow-up, and founding does not read them.

## 3. Found, activate, OpenBatch

1. **Found** through the ordinary founding campaign with `market.json` as the
   market input. The General entry is derived by the same neutral seam Direct's
   is, so nothing about the founding driver changes; `input.direct_capability`
   is `None` and the compiler refuses if it is not.
2. **Activate**:

   ```
   dclutch-local-successor-bootstrap devnet-general-capability-activation-v1 \
     --rpc-url "$URL" --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --plan $JOB/plan.json --campaign-report $JOB/campaign-execute.json \
     --payer-keypair $JOB/keys/campaign-payer.json \
     --output $JOB/general/activation.json --execute
   ```

   The campaign report must be a **devnet** one: the route checks the document
   and the endpoint separately against the same value, and a loopback report
   against a devnet endpoint refuses by name. The report it writes carries
   schema `dclutch-devnet-general-capability-activation-report-v1`.
3. **OpenBatch N=2** through the successor, against the activated root. This
   needs cohort-14 specifically: `a517d27c` moved the input bank to the inline
   CPI transport, and cohort-13's Trading was built from `315f1931`, which
   predates it.

## What is proved, and where

| claim | instrument | status |
| --- | --- | --- |
| the accelerator ELF is reproducible | two detached worktrees at `324528a4` | **byte-identical** |
| it is deployed, and the chain carries exactly those bytes | `solana program dump` compared to the build | **identical** |
| its slot and authority are what this doc says | hostile decode of the finalized ProgramData header | **read, not transcribed** |
| the record minted over it authenticates its own observation | `ArtifactReleaseV1::authenticate_deployment`, offline, inside the compiler | **checked at compile** |
| the Registry ADMITS that observation at finalization | `record_v1.rs::devnet_general_accelerator_observation`, the program's own partition over the real devnet facts | **green** |
| each substitution refuses by its own code | same module: `ArtifactReleaseNotDeployed`, `ReleaseSuperseded`, `ArtifactReleaseElfMismatch` | **green** |
| the compiler's output founds a manifest the founding validator admits | `general_market::the_founding_validator_accepts_a_general_selected_market_input` | **green, static gate** |
| the DEVNET compiler's output does, over the real accelerator's release id | `general_devnet_market::the_devnet_compiler_founds_a_general_market_over_the_real_accelerator_release` — the whole compile minus the two chain reads, against the devnet flagship graph | **green** |
| swapping the accelerator moves the manifest entry | same test: one flipped bit of the artifact release moves both `release_id` and `config_id`, and leaves the capacity profile alone | **green** |
| the activation arm takes both cluster checks | `general_capability_activation::each_arm_takes_both_the_document_check_and_the_endpoint_check`, proved red under a hardcoded cluster | **green** |
| and takes them through the real binary, not only the unit test | three invocations, below | **green** |
| a devnet Registry finalizes the record | — | **owed, cohort-14** |
| the Core program admits a General manifest on a real chain | — | **owed, cohort-14** |
| OpenBatch N=2 executes against this accelerator on devnet | — | **owed, cohort-14** |

## What is still owed, said plainly

1. **The three named identities commit to evidence, not to this ELF.**
   `compiler_release`, `toolchain` and `translation_validation` are now digests
   of real files with stated headers instead of projections of a release-set id,
   which is a narrowing and not a fix. Nothing checks that the toolchain file
   describes the build that produced `61b2d73d…`. The shape that would:
   `dclutch-release-tool` already carries `source_digest`, `cargo_lock_digest`,
   `source_revision`, `solana_version`, `cargo_build_sbf_version`,
   `target_triple` and `build_command` in `BuildMetadataV1` and never hashes
   them into a standalone identity. Deriving `toolchain` and `compiler_release`
   there, from a `CheckedReleaseV1` minted over this exact artifact, closes it.
   `SERIES_SHADOW_COMPILER_RELEASE_PREIMAGE_V4` is the tree's one named
   compiler-release constant and is referenced nowhere.
2. **`translation_validation` is Direct-shaped.** The 21 evidence inputs are
   Lean Direct semantics against the Rust interpreter and AOT. There is no
   General translation-validation corpus, so the honest devnet General market
   names Direct's, and that is a claim about a different program.
3. **The accelerator has no protocol-owned semantic release identity** — §0.
4. **The external widths are stated, not observed.**

## The commands, run

Nothing covered the CLI wiring, so it was run. Family flags do not cross:

```
$ ... devnet-general-market --direct-fee-basis-points 50
Error: unknown devnet-general-market argument: --direct-fee-basis-points
$ ... devnet-sponsored-market --general-policy /tmp/x.json
Error: unknown devnet-sponsored-market argument: --general-policy
```

An omitted accelerator authority refuses with the reason, not the flag name:

```
Error: --general-accelerator-upgrade-authority is required: pass the key the
deployment must be upgradeable under, or the literal `immutable` to assert it
carries none. An omitted flag would mint an Immutable release for a mutable
program
```

And the activation route's two checks are each reachable and each named:

```
$ ... devnet-general-capability-activation-v1 --rpc-url http://127.0.0.1:8899/ ...
Error: public executor requires acknowledged Solana devnet and refuses loopback
$ ... devnet-general-capability-activation-v1 --rpc-url https://api.devnet.solana.com/       --i-mean-devnet <devnet genesis> ...          # with a LOOPBACK campaign report
Error: terminal evidence requires an executed external devnet campaign; loopback
and preflight reports are non-consumable
$ ... local-private-validator-general-capability-activation-v1 --rpc-url http://127.0.0.1:8899/ ...
Error: campaign report omitted genesis_hash          # cleared both, refused later
$ ... local-private-validator-general-capability-activation-v1 ... --i-mean-devnet <hash>
Error: REFUSED: [input/unknown-flag] --i-mean-devnet
```

**Running it is what found the ordering defect.** Before the last commit all
three of the first invocations reported `successor plan: missing field schema`:
the plan was decoded ahead of both cluster checks, so the two refusals a caller
most needs to see were shadowed by a third. Not a safety hole — nothing had
connected — but the unit test could not have caught it, because the unit test
calls `authenticate_cluster_v1` directly and never sees what precedes it.

## One thing this lane expected to be true and measured otherwise

`GeneralConfigV3` carries no deployment field — capacity, claim basis,
program-set identity, generation, price scale, windows, selection policy,
beneficiary — so the obvious expectation is that changing the accelerator moves
the entry's `release_id` and leaves its `config_id` alone. It does not. The
config binds `program_set_id`, and the program set is downstream of the
certificate that names the accelerator, so **one flipped bit of the artifact
release moves the whole entry**. The test asserted the wrong thing first and was
corrected by the run.

This does not disturb why a General market is foundable where a Fractional one
is a fixed point. The dependency is

```
accelerator -> certificate -> strategy -> descriptor -> program set
            -> config -> manifest -> Market
```

strictly one way, and no step reads the Market. Acyclic, exactly as the
selection seam's header claims. What it does mean is that the accelerator
identity is not a detail hanging off the side of a General selection: it is a
seed of the Market PDA, and a cohort that founds against the wrong accelerator
founds a different market, not the same market misconfigured.
