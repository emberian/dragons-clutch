# The Direct capability-activation wall, measured on the live flagship

2026-08-29, TRADE lane. This document records why the first Direct trade on
the public devnet flagship `7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC`
is refused by the deployed protocol itself, with every refusal point read in
the sealed artifacts and the cohort-6 program source. Nothing here is a
client defect: the trade exterior, producers, and route manifest emitters all
work up to this wall.

## The chain of demands, each verified at its owner

1. **A trade needs the capability root.** The Trading Hot leg reads the root
   account's `CapabilityRootHeaderV1` and its exact 24-byte
   `DirectRootStateV1` tail before executing anything
   (`programs/dclutch-trading-sbf/src/hot_v3.rs` root prestate reads), and the
   operator's route authentication requires the root account Trading-owned at
   planning (`crates/dclutch-operator/src/direct_inline_route_v3.rs`,
   `authenticate_named_route_v3`). The root
   `Ht5td43wzV6wMmj3efghMeuWMzNscmnvhJr44gK3XQvu` does not exist on devnet
   (AccountNotFound at finalized commitment, 2026-08-29 ~20:20 EDT).

2. **Only activation creates the root.** Core's capability route
   (`programs/dclutch-core-sbf/src/capability.rs`,
   `CoreEffectActionV1::ActivateCapability`) CPIs Trading's outer
   (`programs/dclutch-trading-sbf/src/outer.rs`, `process_activation`), which
   writes `CapabilityRootHeaderV1 || <family tail>`. Nothing at founding
   creates it; no other instruction can.

3. **Activation selects a descriptor out of the family's ProgramSet and
   demands two facts of it:**
   - the selected descriptor's schema must be
     `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1` (`outer.rs:948`,
     `TradingSbfError::UnsupportedContent`);
   - the paired effect must project exactly the root tail:
     `effect.request_bytes() == descriptor.root_state_bytes()`
     (`outer.rs:1442`, `TradingSbfError::Root`). For Direct that is
     `DIRECT_ROOT_STATE_BYTES_V1 = 24`.

4. **The sealed flagship ProgramSet has no such entry.** Decoded from the
   sealed `market17.json` (the exact bytes the founding published as
   `direct_program_set_record`, content `2e18e9ef…`): 248 bytes = header + 3
   entries:

   | selector | descriptor schema | descriptor `root_state_bytes` | effect `request_bytes` | activation verdict |
   |---|---|---|---|---|
   | `InlineOrdinary` | CapabilityProgramV4 | — | — | refused at `outer.rs:948` (not V1) |
   | `DIRECT_BEGIN_RETIRING_SELECTOR_V1` | V1 (`DCLTCPR1`) | 24 | **0** | refused at `outer.rs:1442` |
   | `DIRECT_NATIVE_CLOSE_SELECTOR_V1` | V1 (`DCLTCPR1`) | 24 | **0** | refused at `outer.rs:1442` |

   (Descriptor `root_state_bytes` read at `CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET
   = 272`; effect `request_bytes` at effect-kernel v2 offset 14. An earlier
   sweep misread the descriptors as declaring zero — they declare 24; it is
   the sealed *effects* that project nothing.)

5. **The gap is sealed into the market identity.** The manifest entry 0
   (kind `DIRECT_SUCCESSOR_KIND_ID_V3`, activation policy PrepaidLazy,
   zero dependencies) binds `release_id = sha256(program_set bytes)`, and the
   manifest digest is one of the Market PDA's own identity seeds. No record
   republication can add a fourth entry to THIS market.

6. **The deadline is real and enforced twice.** Entry 0's
   `activation_deadline_slot = 490330281` (about 16:30 EDT 2026-08-30) is
   checked at `capability.rs` (`current_slot > deadline` refuses) and inside
   `FundingLedgerV2::activate_in_place`
   (`Error::ActivationDeadlineElapsed`). When it lapses, the Trading funding
   ledger `2zQPU4Lq…` (Pending, 4,398,720 lamports = exact rent + the
   2,672,640-lamport root quote) can never activate: `7Mcu1ZT9` remains Open,
   admittable, and permanently untradeable.

This is the deferred work the repository already names:
`docs/OMISSION_INDEX.md` ("Still required for a live release: an eighth
`CapabilityProgramSetV2` entry naming the activation descriptor"),
`tools/gauntlet/blocked.json`, and the web route census
(`UNSELECTED_ENTRY_ROUTES`). No capability of ANY family has ever been
activated on any cluster; the Core/Trading activation route is itself
first-execution territory.

## What unblocks the first trade, exactly

1. **Author the Direct activation artifact triple** in
   `crates/dclutch-direct-codec` (a sibling of `begin_retiring_bundle_v1` /
   `native_close_bundle_v1`): a V1 `CapabilityProgramV1` descriptor with
   `root_state_bytes = 24`, an `AccountProfileV1`, and an effect-kernel-v2
   `EffectProgramV2` whose projected request buffer is the exact initial
   `DirectRootStateV1` (`DCLTDRT1`, version 1, phase Open, count 0 — 24
   bytes). This is protocol-artifact authoring and deserves adversarial
   review; a wrong effect activates a wrong root permanently.
2. **Add the fourth ProgramSet entry** naming it
   (`crates/dclutch-direct-codec/src/program_set_v4.rs`,
   `ordinary_lifecycle_entries`), at a non-colliding selector.
3. **Found a successor flagship** from the regenerated market input (the
   cohort-6 PROGRAMS are data-driven and need no redeploy; the founding,
   admission, and trade drivers are all proven or landed).
4. **Drive the activation** with the frame mapped below, then the trade.

## The complete activation frame (mapped for the driver)

Single Core instruction, permissionless (fee payer only; the Core caller
authority is a PDA Core itself signs via CPI — it must NOT be a top-level
signer). Instruction data:
`Request::administrative(ActivateCapability, generation, market)` (72 B) ‖
`CoreEffectEnvelopeV1` (280 B; binds sha256 of the live 360-byte Market
account and sha256 of the role request) ‖ role request
(`CapabilityExecutionSelectionV1` 144 B with record bumps ZERO ‖
`CapabilityFundingHeaderV2` 16 B — for the flagship shape:
physical=1, logical=1, mask=0b0001 ‖ family request ≥16 B with the u32 set
selector at offset 12).

35 accounts for the Direct P=1 shape (Market W, realm record pair, manifest
record pair, the Trading funding ledger W, the root W, activation cache,
core/trading/resolution program+programdata, registry program, rent sysvar,
caller-authority PDA, then the child tail: program-set pair, config pair,
activation profile pair, activation effect pair, seven exact aliases of the
prefix accounts, system program, activation descriptor pair). Exceeds the
1,232-byte packet uncompressed — needs one ALT. Root PDA seeds:
`dclutch:capability-root:v1`, market, generation LE, manifest, entry index
LE, kind, capability release, config — under Trading. Funding ledger PDA:
`dclutch/cap-funding-ledger/v2`, controller program, market, generation LE,
manifest, mask LE — under the controller. The closest working template is
the Core-level CloseCapability builder in
`programs/dclutch-core-sbf/tests/capability_close_alias_program_test.rs`.

## What the TRADE lane landed against this wall

- `devnet-checked-execution-release-v1` and
  `devnet-direct-hot-route-manifest-v3` (commit `1fe37fa4`): the named
  manifest-producer gap is closed at the emit side, gated on a session's
  frozen lookup table with bracketed refusal codes; the first live-set
  checked multiprogram was produced from the chain's activation cache
  (execution set `d5aaadea…`, checked id `bdf54c41…`).
- Four first-execution walls in `devnet-direct-trade-produce-v1` fixed at
  their owners (`8dfd2a41`, `1bbb8cdf`): transaction-row identity,
  checkpoint-scalar capability root, devnet market-record resolution,
  Open-Market generation.
- Bootstrap tickets, session runner, SDK verification script, and the
  devnet load-simulator configuration staged; all of it runs the moment the
  activation entry exists on a successor market.
