# Literal-sweep convictions — 2026-08-31

Full candidate table from the CANON lane's literal-leftover sweep (S2 of
CANONICAL_CLIENT_EXPECTATIONS_V1.md, which carries the disposition rules and
the adjudications). CONVICTED set owned by the LITERALS lane; SUSPECT set
needs the emitters briefed in that doc. Line numbers as of 54888c5e.

Sweep complete. Candidate table below.

## CONVICTED — hard drift (live refusal risk)

**1. Core Market state magic/version pinned to a superseded generation**

| | |
|---|---|
| `/Users/ember/dev/dclutch/apps/dclutch-web/lib/directHotChain.ts:911` | `… market.data.length !== CORE_STATE_BYTES \|\| ascii(market.data, 0, 8) !== 'DCLTCOR2' \|\| u16(market.data, 8) !== 2 \|\| market.data[10] !== 1` |
| Twin | `CORE_STATE_MAGIC` = `DCLTCOR3` and `CORE_VERSION = 3` — `apps/dclutch-web/lib/generated/coreFound.ts:35, :4` |

The same function already imports `CORE_STATE_BYTES` from that exact module (line 10 of the import block). The literal `'DCLTCOR2'`/`2` sits beside its imported sibling. `marketCoreV2.ts:273-275` is the correct implementation (`ascii(CORE_STATE_MAGIC, 0, 8)` / `version !== CORE_VERSION`) — and its own comment at `marketCoreV2.ts:204-219` documents this exact defect having already bitten once on width. Also `market.data[10] !== 1` has twins `CORE_STATE_PHASE_OFFSET = 10` / `CORE_PHASE_OPEN_TAG = 1` (generated/coreFound.ts:37, :54).

Same defect, second site: `/Users/ember/dev/dclutch/apps/dclutch-web/lib/dealerEquityChain.ts:295-296` (imports `CORE_STATE_BYTES` at line 20).

SDK copies: `packages/dclutch-sdk/lib/directHotChain.ts:896` (file otherwise **differs** from web — web has ~15 extra lines), `packages/dclutch-sdk/lib/dealerEquityChain.ts:295` (identical twin).

**2. AccountProfile operation stride as bare `16`**

| | |
|---|---|
| `/Users/ember/dev/dclutch/apps/dclutch-web/lib/directInlineV3.ts:654` | `const expectedProfile = profileHeader + (fixed + stride) * RULE_BYTES + (fixedOperations + itemOperations) * 16;` |
| Twin | `OPERATION_BYTES = 16` — `apps/dclutch-web/lib/generated/directInlineV3.ts:199` |

`HEADER_BYTES` and `RULE_BYTES` are imported from that module in the same import list; `OPERATION_BYTES` appears **zero** times in the file. Purest form of the shape. Same line in SDK (files identical).

**3. Dealer LP Position width as bare `256` inside a chain validator**

| | |
|---|---|
| `/Users/ember/dev/dclutch/apps/dclutch-web/lib/dealerAccountProfileV3.ts:504` | `\|\| profile.rules[DEALER_LP_STATE_ACCOUNT_V3]?.dataLength !== 256` |
| Twin | `DEALER_LP_POSITION_BYTES_V3 = 256` — `apps/dclutch-web/lib/generated/dealerEquityV3.ts:29` |

`DEALER_LP_STATE_ACCOUNT_V3` on that very line is imported from the module that defines the twin (~50 siblings imported). SDK identical at `:504`.

## CONVICTED — shadow constants (values currently agree; drift-exposed)

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/rationalRetireReceiptV4.ts:76-82` redefines by hand what `import * as Hot from './generated/directInlineV3'` (line 36) and `./generated/coreFound` (line 34) already supply:

| Local | Emitted twin |
|---|---|
| `CLAIMS_MARKET_HEADER_BYTES = 256` (:77) | `LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 = 256`, generated/coreFound.ts:65 |
| `CAPABILITY_ROOT_HEADER_BYTES = 232` (:79) | `CAPABILITY_ROOT_HEADER_BYTES_V1 = 232`, generated/directInlineV3.ts:103 |
| `CAPABILITY_SET_HEADER_BYTES = 32` (:80) | `CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 = 32`, generated/directInlineV3.ts:146 |
| `CAPABILITY_SET_ENTRY_BYTES = 72` (:81) | `CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2 = 72`, generated/directInlineV3.ts:147 |
| `CAPABILITY_PROGRAM_V4_BYTES = 600` (:82) | `CAPABILITY_PROGRAM_V4_BYTES = 600`, generated/directInlineV3.ts:77 — *identical name* |

SDK copy at `packages/dclutch-sdk/lib/rationalRetireReceiptV4.ts:75-81` (file otherwise differs from web).

Same file, magic strings whose twins live in the already-star-imported module:
- `:362` `ascii(bytes,0,8) !== 'DCLTCRT1'` → `CAPABILITY_ROOT_MAGIC_V1` (generated/directInlineV3.ts:276)
- `:369` `'DCLTCER1'` → `CAPABILITY_EXECUTION_SELECTION_MAGIC_V1` (:277)
- `:392` `'DCLTCPS2'` → `CAPABILITY_PROGRAM_SET_MAGIC_V2` (:273) — `directHotChain.ts:507` does this correctly via `DirectAbi.…`, so this is the outlier
- `:435` `'DCLTCPR4'` → `CAPABILITY_PROGRAM_V4_MAGIC` (:272) — `directHotChain.ts:562` correct
- `:497` `'DCLLBM02'` → `LIABILITY_BASIS_MARKET_MAGIC_V2` (generated/coreFound.ts:62)
- `:516` `'DCLRNTL2'` → `LIFECYCLE_RENT_CREDIT_MAGIC_V2` (generated/coreFound.ts:30) — the line already uses `LIFECYCLE_RENT_CREDIT_BYTES_V2` from that same module
- `:882` `encode('DCLTHOT3')` → `HOT_EXECUTION_MAGIC_V3` (generated/directInlineV3.ts:270)

**Claims aggregate / Position decode built entirely from literals while the full offset set is emitted** — `/Users/ember/dev/dclutch/apps/dclutch-web/lib/rationalOpenChainV4.ts:155-179` (SDK identical):

`if (bytes.length !== 256 + input.outcomes * 8 || ascii(bytes, 0, 8) !== 'DCLLBM02' || u16(bytes, 8) !== 2 || u32(bytes, 12) !== input.outcomes)`

The file imports `LIABILITY_BASIS_MARKET_SEED_V2`/`LIABILITY_BASIS_POSITION_SEED_V2` from `./generated/coreFound` (lines 10-13) — the same module that emits every literal used here: `LIABILITY_BASIS_MARKET_HEADER_BYTES_V2` (256), `..._MAGIC_V2`, `LIABILITY_BASIS_STATE_VERSION_V2` (2), `..._CLAIM_COUNT_OFFSET` (12), `..._REVISION_OFFSET` (16), `..._LOGICAL_ID_OFFSET` (24), `..._RELEASE_SET_OFFSET` (56), `..._REGISTRY_OFFSET` (88), `..._PRODUCT_OFFSET` (120), `..._BASIS_OFFSET` (152), `..._REALM_OFFSET` (184), `..._CUSTODY_CONTEXT_OFFSET` (216), `..._GENERATION_OFFSET` (248). Position path at `:173-179` mirrors it against `LIABILITY_BASIS_POSITION_*` (128, 12, 16, 24, 56, 88, 120).

Same file `:200` — `ascii(profile, 0, 8) !== 'DCLTAP02' || u16(profile, 8) !== 2 || u16(profile, 10) !== 11`:
- `'DCLTAP02'` → `ACCOUNT_PROFILE_MAGIC_V2` (generated/dealerEquityV3.ts:93) / `MAGIC` (generated/directInlineV3.ts:291)
- `2` → `ACCOUNT_PROFILE_VERSION_V2` (generated/dealerEquityV3.ts:42)
- `11` → `AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE: u16 = 11` (`crates/dclutch-account-profile-contract/src/v2.rs:71`) — no TS twin emitted yet
- `:52-54` `ACCOUNT_PROFILE_HEADER = 40` → `AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES: usize = 40` (v2.rs:99); `ACCOUNT_PROFILE_RULE = 16` / `ACCOUNT_PROFILE_OPERATION = 16` → `ACCOUNT_PROFILE_RULE_BYTES_V2` / `ACCOUNT_PROFILE_OPERATION_BYTES_V2` (generated/dealerEquityV3.ts:58-59)

Also `directInlineV3.ts:631` — `decode(profile.slice(0, 8)) !== 'DCLTAP02'` with `view.getUint16(8) !== 2` and `[2, 3, FIXED_DATA_PREDICATE_ARTIFACT_PROFILE]`: the third element is imported from generated, the first two (`ARTIFACT_PROFILE = 2`, `SELECTED_WINDOW_ARTIFACT_PROFILE = 3` in v2.rs:48,50) are literals.

## SUSPECT

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/localSuccessor.ts:182` — whole Resolution-certificate decoder written in literals: `bytes.length !== 312`, `u16(bytes,8) !== 1`, `bytes[10] < 1 || > 4`, `requireZero(…,11,5)`, `requireZero(…,260,4)`, and reads at 16/240/248/252/256/264/272/280/296/304. Every one of those numbers has a named twin: `RESOLUTION_CERTIFICATE_BYTES_V2` and the full `CERTIFICATE_V2_*_OFFSET` set in `crates/dclutch-resolution-codec/src/generated_v2.rs:19-45`, mirrored into TS as `packages/dclutch-sdk/lib/generated/resolutionCertificateV2.ts`. `packages/dclutch-sdk/lib/resolutionCertificateV2.ts:58-86` is the reference implementation using all of them. Verdict is SUSPECT not CONVICTED because the magic here is `DCSRCER1` (V1, `formal/dclutch-semantics/DClutchSemantics/SourceResolutionAbi.lean:244-245`) while the emitted TS module is V2 (`DCSRCER2`); offsets are numerically identical across both, and `apps/dclutch-web/lib/generated/` has **no** `resolutionCertificateV2.ts` at all — the web tree cannot import the twin today. The Lean `certificateSchema`/`certificateBytes`/`certificateMagic` are scrapeable for a V1 emitter. Same file `:181` uses literal `1288` for the activation cache while `ACTIVATION_CACHE_BYTES = 1_288` is exported from sibling `releaseRegistry.ts:18`; `:183` (`DCLTSRS1`, 224 bytes, offsets 15/48/184/192/208) has twins in `crates/dclutch-source-contract/src/generated_source_resolution_state_v2.rs` — I did not byte-verify the V1/V2 offset correspondence. SDK copy differs from web but carries the same block.

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/dealerAccountProfileV3.ts:415-422` — header field reads at 12/14/16/18/20/22/24/26 (`fixed`, `itemStride`, `fixedOperations`, `itemOperations`, `commonScalars`, `itemScalarStride`, `commonIdentities`, `itemIdentityStride`). I looked in `generated/dealerEquityV3.ts`, `generated/directInlineV3.ts`, `crates/dclutch-account-profile-contract/src/v2.rs` and `.../generated.rs`. The **V1** file emits `ACCOUNT_PROFILE_ACCOUNT_COUNT_OFFSET = 12` … `ACCOUNT_PROFILE_IDENTITY_COUNT_OFFSET = 18`, but those V1 names carry different field semantics than the V2 layout being read here, and no V2 header-offset constants are emitted anywhere. Twin plausible, not found. Sibling literals in the same validator with no twin found: `:490` `fixedOperations !== 3`, `:502` `fixedOperations !== (open ? 14 : 13)`, `:473` `custody = add ? 2 : 3`.

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/rationalCapabilityChainV4.ts:73` — `bytes.length < 104 || ascii(bytes,0,8) !== 'DCRRGRP2' || u16(bytes,8) !== 2`, plus `:76` `requireZero(bytes,92,4)`, `:82` `u64(bytes,96)`, `:85` `104 + index * 64`. No `DCRRGRP2` twin in any generated TS module or `crates/*/src/generated_*.rs`; I checked `dclutch-rational-representation-v2-kernel/src/generated_descriptor.rs` and `generated_product_v3.rs`. Same for `:193` `rootAccount.data.length !== 232 + capability.rootStateBytes` (the 232 does have a twin — `CAPABILITY_ROOT_HEADER_BYTES_V1`, generated/directInlineV3.ts:103 — but this file does not import that module).

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/directHotChain.ts:843` — Direct capability seal: `'DCLTCSL1'`, `u16(seal.data,12) !== 6`, `u16(seal.data,14) !== 0x00ff`, `readU32(seal.data,16) !== 1`, identity slices at 24/56/88/120, and the PDA domain `'dclutch:capability-seal:v1'` hand-carried at `:127`. `crates/dclutch-capability-seal-contract` has no `generated_*.rs`; nothing emitted found. The `0x00ff` in particular reads like a wire constant that should be named.

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/dealerEquityChain.ts:164` — `u32(bytes, 12) !== 10 || bytes[16] !== 2 || bytes[17] !== 0` for ProgramSet V1, immediately after using `HotAbi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1` on the previous line. Only `..._HEADER_BYTES_V1`/`..._ENTRY_BYTES_V1` are emitted for V1; no selector-count twin found.

`/Users/ember/dev/dclutch/apps/dclutch-web/lib/rationalOpenChainV4.ts:~231` — `coordinate !== 5 && observed.isSigner !== …`: a bare account-index exemption inside the Profile11 privilege loop. Looked for a route-index twin in `generated/claimsCustodyReplayV1.ts` (`REPLAY_ACCOUNT_*_V1`) and `generated/directInlineV3.ts` (`HOT_*_ACCOUNT_V3`); none matches this frame.

## BENIGN (checked, not reportable)

`bytes.length !== 32` / `!== 64` identity and signature guards throughout (`bytes.ts:47`, `directCodec.ts:52`, `directTransaction.ts:99`, `directTicket.ts:63`) — structural to ed25519/pubkey. Loader layout literals `36` / `45` / `u32(data,0) !== 2|3` (`rationalRetireReceiptV4.ts:765`, `localSuccessor.ts:166-167`) — Solana upgradeable-loader shape, not dClutch ABI. SPL Token-2022 offsets in `rationalTokenV2.ts:157,264-273` and `registeredDirect.ts:264-273` — foreign program layout. Solana native program ids in `activity.ts:29-36`, `rpc.ts:29-31` — canonical. `deployments.ts:91-136` — this file *is* the deployment record; note only that `genesisHash: 'EtWTRABZ…'` at `:91` duplicates `SOLANA_DEVNET_GENESIS_HASH_V1` exported from sibling `rpc.ts:29` (local twin, not generated). `components/*.tsx`: only `RationalOpenPanel.tsx:42` and `RationalRetireReceiptPanel.tsx:36` (`addresses.length !== 38`) carry ABI-shaped literals — the 38-account Hot frame; the sibling `rationalOpenClaimsMetasV4` comment calls it "32+4*N", so the count is derived, not fixed — flagging as low-confidence SUSPECT rather than convicted.

**Files with no findings:** `capabilityManifest.ts`, `realmRecord.ts`, `refusals.ts`, `productRuntimeV2Admission.ts`, `claimsCustodyReplay.ts`, `marketCoreV2.ts` (reference-quality), `generalPlanV5.ts` and `infrastructure.ts` (near-clean; `generalPlanV5.ts:345,409,428` use `u16(bytes,8) !== 2` beside `Abi.*_MAGIC_V2` where a `*_VERSION_V2` twin was not emitted), `explorer/**`, `founding/**`.
