# Structured receipt: external Token runtime localization

Observed 2026-09-09 UTC. This is an actual local-validator refusal followed by
ProgramTest replay. No Structured receipt has yet been accepted on the retained
validator, and no devnet transaction was signed or submitted for this measurement.

The actual host was `9441be63001ce661385cec3e470e61b4035993c8`, with checked
eight-program runtime `bf06c8d752eafa572a433b49cb0db38fe413fbbd`. The work directory
is `hbox:/tank/dregg-build/structured-validator-9441be630-20260909`.
Root activation accepted at slot 12963 (536633 CU), receipt seal at slot 13158
(147392 CU). Receipt Hot reached Claims, allocated the mint, and initialized its
close authority, then Token-2022 refused PermissionedBurn initialization with
`spl_token_2022_interface::error::TokenError::InvalidInstruction`. Atomic preflight
left no receipt mint and no resource-counter increment.

## Exact external deployment and replay

The canonical external program identity is
`TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`. Agave automatically loaded a
506896-byte ELF with SHA-256
`a794161408080f690dac00832f45b3c3e2b71f1339586667ad1f979cf91d5b68`.
The old shared launcher supplied no explicit Token program.

Two authorized, bounded finalized `getAccountInfo` calls to
`https://api.devnet.solana.com` observed slots 495389360 and 495389362. The actual
devnet program is executable and owned by Loader V3, pointing to ProgramData
`DoU57AYuPFu2QU514RktNPG22QhApEjnKxnBcu4BHDTY`, deployment slot 461026092,
upgrade authority `3URRPr96EV2wuNRgQKwQpuZitHHsVyDUen1eRSvEun9G`. Its 711008-byte
code region has SHA-256
`0f3038f9271a900ffd562ffd974934ca8a9c8f8d048f67dbd39aebb35e5b83fb`.
The RPC images, binary, and facts are retained under
`hbox:/tank/dregg-build/structured-token-devnet-20260909`. A semantic version or
reproducible upstream source identity for this public deployment was not inferred
from its address or strings.

The canonical local fixture is independently pinned by
`programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance`: spl-token-2022
11.0.0, upstream `d9a5ce37c018981b6823746856ff9fe1268837cf`, platform-tools 1.53,
615704-byte ELF SHA-256
`e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697`.

Each replay used the same captured receipt, first-party ELFs, and account bodies;
the green variants changed only the external Token ProgramData ELF tail.

| External ELF | Result | Total CU | Trading CU |
| --- | --- | ---: | ---: |
| Actual Agave a794… | Exact original Token InvalidInstruction | 367439 | 366831 |
| Canonical v11 e2ac… | `Some(Ok(()))` | 397113 | 396505 |
| Fetched devnet 0f3038… | `Some(Ok(()))` | 397137 | 396529 |

Both green logs contain accepted `PermissionedBurnInstruction::Initialize`, then
successful mint initialization, Claims, and Trading. This proves the fetched
devnet code supports the required receipt initialization under this captured
frame; it is not evidence that the entire lifecycle was submitted on devnet.

## Capture and instrument boundary

Artifacts are under
`hbox:/tank/dregg-build/structured-host-9441be630-completion`:

| File | SHA-256 |
| --- | --- |
| `receipt-token-frame-9441.json` | `684f373a3e7c3b54365c285f83e857464820ce186d9d6560a0be91226e8f3a9b` |
| `receipt-token-frame-with-programdata.json` | `f6dda20f511fc44ea12dbd5be89a8c19bb910a29009fc8bbb816a4a8ae0bb324` |
| `receipt-token-replay-normalization.json` | `c2bb6d0ad252674c20e2c4203347d7161e1ab22c1f588514797c64bb2a27e349` |
| `receipt-token-old-replay-normalized.log` | `18a362a0f030f4fb2711aae7f5d27def0c025d8c4fab4637bb547162e3201555` |
| `receipt-token-canonical-replay.log` | `da07dae7096ae892b58e5f3ecffc2f4981f8c25b318a4b2fdd3ff17ea9cb7931` |
| `receipt-token-devnet-replay.log` | `9ae3108bcee252b538d5bdda4097dc05a7b8c190d9016c3c614f4dd9bc5b53d3` |

The original capture finalized at 22408 and its native account-profile projection
returned `Ok(())` with no width or privilege mismatches. The derived capture adds
the separately captured actual Token ProgramData account, which is loaded code
rather than an instruction account. The bf06 ProgramTest replay executable is
retained at `structured-native-bf06c8d75-20260908/source/target/debug/`.
Its measured fork limitation requires Core/Claims/Token deployment slots 7/5/0
to become 6, the corresponding mutable activation-cache slots to become 6, and
the frozen ALT's last-extended slot to become zero. Every such normalization is
listed separately; original captures and actual runtime source remain unchanged.
The old-ELF control reproduces both the original refusal and CU count exactly.

Capture resumed with a host-only correction: campaign `founding_custody_context`
is the projected founding namespace, not the normal namespace persisted by
Claims. Comparing them rejected honest state. Canonical aggregate owner/PDA,
Core identity, release, Registry, Realm, and generation authentication remain;
the invalid cross-namespace equality is removed. The capture build was an owned
9441 checkout with the previously checked reclaim-order host overlay and this
one-line correction. The actual 9441 runtime checkout was not edited.

## Convergence

Structured now requires an explicit external code pin before substrate
publication. `DCLUTCH_TOKEN_2022_ELF` selects the fetched public deployment,
authenticated against `tools/gauntlet/structured-claims/token-2022-devnet-20260909.json`;
`TOKEN_2022_V11_ELF` selects the distinct canonical v11 fixture. Conflicting
selectors refuse. The shared local launcher authenticates the selected file,
installs it only at fresh genesis, records `external-token-runtime.json` outside
the eight-link gate, and authenticates the loaded executable/code before any
administration publication. This is explicit local fixture evidence and grants
no authority to deploy or replace the public Token program. The narrow host
overlay passed `cargo check --locked -p dclutch-journey-campaign` on its own hbox
target (9.09 seconds for the final dual-profile patch); log SHA-256
`44ab4cb571f6444f662337c9be0cd64230b3475ccc1b8a1eb88ffeb4e5366e74`.
No scheduler no-op was reported.

A fresh actual cohort with the corrected Core provider acknowledgement is still
required for receipt, all representation actions, settlement, and retirement.

## 2026-09-09 addendum: Agave disabled-authority encoding

The fresh 691 runtime with host `a93c5c12d` reached genesis code readback and
refused before administration: Agave's `--bpf-program` loaded the correct fetched
devnet ELF but represented its disabled authority as `Some(Pubkey::default())`,
not the required Loader V3 `None`. A controlled restart of the retained genesis
confirmed this at finalized slot 79; ProgramData was 711053 bytes, code SHA-256
`0f3038f9271a900ffd562ffd974934ca8a9c8f8d048f67dbd39aebb35e5b83fb`, header option
tag one followed by 32 zero bytes. The exact RPC image is retained at
`hbox:/tank/dregg-build/structured-host-a93c5c12d-completion/token-genesis-accounts.json`.

The launcher now uses Agave's documented
`--upgradeable-program TokenID ELF none` spelling for this external dependency,
keeping the exact disabled-authority check. First-party prepared deployment
images are unaffected. This is a local genesis argument correction; no public
program was upgraded and no first-party runtime source changed.
