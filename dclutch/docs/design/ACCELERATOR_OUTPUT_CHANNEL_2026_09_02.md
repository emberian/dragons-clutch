# The accelerator's output channel

Reading lane, 2026-09-02, at `afa556f3` (`git rev-parse --show-toplevel` =
`/Users/ember/dev/dclutch`). No source changed. Every path:line below is HEAD.

## 1. The wall, as the code states it

A stateless accelerator has exactly two ways to hand bytes back to Trading:
CPI return data, capped at 1,024 bytes by the runtime, and a writable account
it owns. The tree uses only the first.

- `SVM_RETURN_DATA_BYTES_V2 = 1024`, `ACCELERATOR_ACK_HEADER_BYTES_V2 = 144`,
  `ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2 = 880`
  (`crates/dclutch-execution-strategy-contract/src/generated_v2.rs:10-13`,
  emitted from `formal/dclutch-semantics/DClutchSemantics/ExecutionStrategyV2Abi.lean:47-49`,
  pinned by `:342-344`; `v2.rs:1988-1991` asserts header + payload = 1,024).
- A bank is `scalars×8 + identities×32` (`v2.rs:944-957`) and the chunk count
  is `ceil(bank / 880)` (`v2.rs:1784-1795`). Whether the OUTPUT is chunked is
  decided only by that width (`classify_bank_transport_v2`, `v2.rs:960-975`).
- Trading loops once per chunk, one CPI each, one release-pinned caller
  authority PDA each, and the accelerator re-enters from zero every time
  (`programs/dclutch-trading-sbf/src/admitted_composition_v3.rs:296-348`;
  dealer `programs/dclutch-dealer-accelerator-sbf/src/lib.rs:209-284`; General
  `programs/dclutch-general-accelerator-sbf/src/lib.rs:517-598`).

Bank widths at HEAD, and what the chunker makes of them:

| route | scalars | identities | bytes | chunks | where the counts live |
|---|---:|---:|---:|---:|---|
| Dealer equity Add | 26 | 37 | 1,392 | 2 | `dealer/v3_hot_artifact.rs:993-1013` (6 + 2×9 + 2; 1 + 2×17 + 1 + 1) |
| Dealer equity Remove | 35 | 53 | 1,976 | 3 | same, at three custody slots |
| Dealer scenario trade, N=2 | 99 | 115 | 4,472 | 6 | `dealer/v3_admitted.rs:87-89` (97+N, 115) |
| Dealer LP Open | fits | fits | ≤880 | 1 | one invocation, 288,413 CU (`76f4c9eb`) |
| General OpenBatch, N=2 | 163 | 45 | 2,744 | 4 | `crates/dclutch-general-adapter-contract/src/hot_candidate_v3.rs:61-65` (151 + 6N, 45) |
| General OpenBatch, N=13 | 229 | 45 | 3,272 | 4 | same; commits at 863,027 CU (`5afef490`) |

What a chunk costs, measured by the Dealer lane on real ELFs (`76f4c9eb`,
nine `hot_cu_checkpoint!` marks at `hot_v3.rs:940-1177`): equity Add chunk 0 is
445,816 CU, of which 328,702 (74%) is authentication that is byte-identical
between chunks and 100,064 is the family evaluation that recomputes the same
bank. Only the 880-byte slice differs. The commit's own arithmetic, not a
measurement: one invocation puts the equity route near 1.2-1.3M of 1,399,700;
two never fit. General pays the same class at four chunks (861,908 CU at N=2,
`5afef490`; its callback has `heap_mark!` but no CU checkpoints, so its
per-chunk cost is NOT decomposed anywhere in the tree).

The two refuted proposals stay refuted. There is no writable page the
accelerator could reach: `AdmittedChunkBuffersV4` builds every meta
`new_readonly` (`admitted_composition_v3.rs:449-458`), the module invariant is
"no account-write or child-CPI authority" (`:5-9`), and an input scratch page is
Trading-owned by construction (`validate_input_scratch_pages`,
`:752`; General refuses any other owner at
`general-accelerator-sbf/src/lib.rs:787-793`). And the candidate is dense and
mixed: register 0 of the 37 is the parent request digest
(`v3_hot_artifact.rs:88`), the custody slots interleave content ids with keys
(`v4_equity_accelerator_accounts.rs:1352-1370`), so a uniform index encoding
cannot represent it.

## 2. The options

### (a) An accelerator-owned output page

**Mechanism.** The accelerator writes the whole bank into one account it owns;
its acknowledgement carries only the digest and width it already carries
(`total_bank_digest`, `total_bank_bytes`: `v2.rs:1254-1255`); Trading hashes
the page after the CPI and requires equality with the digest whose producer it
already checks (`admitted_composition_v3.rs:541-546`). Authentication of the
BANK does not move: today Trading trusts the bank because its digest arrived in
producer-tagged return data (`:349-351`), and that is exactly the check that
survives. The page needs no ownership or PDA check to be sound -- a page the
accelerator could not write holds stale bytes, whose hash cannot equal the
digest of the bank it just computed -- but the accelerator must refuse by name
a page it cannot write (wrong owner, readonly, too short) rather than fault.

**Atomicity.** Unchanged. A refused disposition carries no digest
(`v2.rs:1289-1300`) and Trading refuses (`:325-327`); a program error rolls
back the transaction and the page write with it; Trading reads the page
immediately after its own CPI and before any other, so no later writer exists.
The page must not alias any other account in the frame -- the
`parse_accelerator_readonly` pairwise check (`hot_v3.rs:14765-14795`) has to
cover it, because CPI privileges union on a repeated key.

**What "readonly stateless" becomes.** Both accelerators state the invariant
as "never writes an account, invokes a child, or owns protocol state" (dealer
`lib.rs:12`, General `lib.rs:10-11`), decision 0003 states it as "may not
create a second state, claim, custody, or release authority" (`:12-16`), and
the General census MEASURES it by re-reading every observed runtime account and
requiring unchanged bytes (`tools/gauntlet/general/bindings.json:10`). A
self-owned page is a weaker invariant, and it is honest to say so: the
accelerator now holds one account. It is not a weaker AUTHORITY: the page's
bytes are read by exactly one party inside exactly one CPI window and bound by
a digest the runtime attributes to the writer; no route reads it later, it
names no semantic owner, and it can move no lamports. The census keeps its
measurement as "every runtime observation unchanged, plus page bytes ==
digest preimage", which is a stronger test than today's, not a weaker one.

**The prompt's shape, and why not that one.** "Trading creates it via the
System program, Trading pays, ephemeral, closed after" fails three times
against the frame:

1. The Hot fixed frame carries no System program and no fee-paying signer
   (`crates/dclutch-capability-program-contract/src/hot_v3.rs:52-134`: 39
   coordinates, none of them). Trading holds no lamports of its own; the only
   route that creates accounts inside a Trading instruction carries an explicit
   `payer` and `system` account for it
   (`dealer_scenario_checkpoint_v1.rs:385-411`). Adding both to the Hot frame
   moves every coordinate after them.
2. Only the owner can zero a page, so "closed after" is a second accelerator
   route -- or a second CPI to the one route with a reclaim request -- in a
   program whose census says it exposes exactly one (`bindings.json:3`).
3. It adds a System CPI per transaction whose cost this tree has never
   measured (no `create_account`/`allocate` CU figure exists under `docs/`).

**The shape that fits the code: a persistent, client-provisioned page.** Anyone
creates an account owned by the accelerator with a plain
`SystemProgram::CreateAccount` signed by the new key -- no accelerator route,
no Trading involvement, rent paid once by the provisioner and reused for every
subsequent transaction. The client passes it writable; Trading forwards it at
one fixed coordinate. A page is one write lock, so a shared page serializes
its users; a page per Trading root adds no serialization the root's own write
lock does not already impose (the ALT witness: exactly one writable in the
fixed frame, `docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md:31-36`).
Rent reclamation needs a Close route the accelerator does not have; that is a
named debt, not a precondition, because a pooled page is created once.

**Blast radius, by file.**

| layer | what changes |
|---|---|
| Lean `ExecutionStrategyV2Abi.lean` + `EmitExecutionStrategyV2AbiRust.lean` | a third transport: request/ack schema pair (`requestMagic`/`ackMagic` and preimages at `:22-26`, `v2.rs:44-54`), no chunk fields, ack = 144-byte header; `transport_profile` (`v2.rs:388-406`) gains a variant. The request/ack pair IS the transport identity (`v2.rs:139-146`), so this is a new Strategy record content, not a flag |
| `crates/dclutch-execution-strategy-contract/src/v2.rs` | `AcceleratorTransportProfileV2::OutputPageV3`, request/ack V3 codecs, hostile decode tests; `ScratchPageKindV2::Candidate` already exists with no producer (`v2.rs:1493-1512`, "Candidate AOT output assembled by Trading") and is the page header if one is wanted |
| `crates/dclutch-execution-strategy-contract/src/admitted_v3.rs` | one page coordinate appended after `ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3` (`:106-108`); `ADMITTED_RUNTIME_ACCOUNTS_START_V3` (`:111`) moves by one. Appending keeps every existing fixed coordinate; both accelerators and Trading derive from these (`admitted_composition_v3.rs:73-116`) |
| `programs/dclutch-trading-sbf/src/admitted_composition_v3.rs` | `execute_admitted_aot_v3` loses the loop (`:296-348`); `admitted_caller_authority_count_v3` (`:173-192`) returns 1 under the new profile; one writable meta in `AdmittedChunkBuffersV4::new` (`:449-458`); page read + digest check after `invoke_admitted_chunk` |
| `programs/dclutch-trading-sbf/src/hot_v3.rs` | frame carving at `:3785-3835` (caller-authority span becomes one account plus the page), the join at `:4047-4060`, page privilege/uniqueness checks in the parse at `:14765`; `hot_admitted_runtime_accounts_start_v3` (`:690-700`); `validate_input_scratch_pages` is untouched -- input transport is orthogonal (`:245-248`) |
| `programs/dclutch-dealer-accelerator-sbf/src/lib.rs` | write bank to page instead of slicing (`:255-282`); new refusal for an unwritable page in band `0xD0xx` (`:53-86`) |
| `programs/dclutch-general-accelerator-sbf/src/lib.rs` | same at `:544-597`; `validate_frame` count (`:627-632`); new refusal in `0xC0xx` |
| host bundle builder | one request and one PDA instead of one per chunk (`program-test/bundle-builder/src/admitted.rs:199-276`); the page as a built account; frame count (`bundle.rs`) |
| TS/WASM twins | `apps/dclutch-web/lib/generalPlanV5.ts:573` hand-counts `FIXED + 8 + scratchPageCount` (the +scratchPageCount is caller authorities: one per page today); `GeneralWorkspace.tsx:161` displays it; `generated/refusalRegistryV1.ts:294-299` regenerates. Neither `AcceleratorAckV2` nor the chunk constants has a TS twin -- the browser never decodes an ack |
| operator | `crates/dclutch-operator/src/general_hot_v3.rs:2054-2070` (page count → authority count), `tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs:634-645` |
| extent pins and census | `open_batch.rs:705-709` has a literal `47 +`; the Dealer rows in `docs/design/PACKET_LIMIT_2026_09_01.md:119` (2,342 / 2,375 / 3,084 legacy) and the ALT identity `accounts(258) − accounts(1) = 2 × pages` (`GENERAL_ALT_PACKET_WITNESS:112-116`) become `1 ×`; `tools/gauntlet/general/witnesses.json` re-records; every Strategy/Certificate/Admission record re-digests, as GEN-SEVEN did (`hot_candidate_v3.rs:54-60`) |

**CU, against the ladder.** One invocation instead of `chunks`: equity Add
saves one whole 445,816 chunk; scenario saves five; General N=2 saves three of
an unmeasured per-chunk cost. Added: a `memcpy` of the bank into the page, one
more account serialized into the CPI frame, and one `sha256` over 1,392 bytes
in Trading -- none measured here, all small next to a chunk. Packet: minus
`(chunks − 1)` caller-authority accounts, plus one page (ALT-eligible).

### (b) A third register space: FrameReference

The V2 AccountProfile vocabulary (Lean-owned since `38b8429c`,
`AccountProfileV2Abi.lean:278-320`) already has `OP_PROJECT_KEY_V2 = 2`
(`generated_abi.rs:79`): an account key into an identity register, in the
INPUT direction. FrameReference is its inverse on the output wire: an identity
that is a frame key encodes as its coordinate. The effect kernel addresses
registers only (`IdentityCoordinateV3::common/item`,
`crates/dclutch-effect-kernel/src/v3/encode.rs:83-100`), so Trading would
expand references into 32-byte registers before projection; the wire bank
becomes a second encoding and `total_bank_digest` is over that encoding, which
all six parties must then share.

What it saves, counted from `project_custody_registers`
(`v4_equity_accelerator_accounts.rs:1352-1370`) and the custody frame roles
(`crates/dclutch-custody-contract/src/frame_spec_v1.rs:73-95`):

- Equity Add, 37 identities: frame keys are Market, CallerProgram, Source,
  Destination, Mint, TokenProgram in both slots (12), the external SourceOwner
  in slot 0 (1), the external delegate and the evidence owner (2) = 15; Payer
  and RentRefund are zero for a transfer (4) and would need a null reference;
  the remaining 18 are content ids (parent digest, and per slot ReleaseSet,
  Realm, Context, Candidate, Order, the vault contexts). With u32 references:
  208 + 18×32 + 19×4 = 860 bytes. Fits one chunk with 20 bytes to spare --
  a margin one added identity erases.
- Equity Remove: 35 scalars alone are 280 bytes; with three slots the content
  ids alone (1 + 3×7 + 1 = 23) are 736 bytes; 1,016 > 880. Does not fit.
- General OpenBatch: 163 scalars are 1,304 bytes before any identity. No
  identity encoding brings it under 880; at best 4 chunks become 3.

So (b) is a bank diet that fits exactly one route, at zero margin, and does
nothing for the route named in the question. It is also the encoding whose
element type is register-dependent, which is the objection already on record.

### (c) Chunking without re-authentication

The accelerator holds nothing between invocations, so a memo would have to
travel in return data: its own previous ack (runtime-tagged with its program
id, unforgeable by Trading) carrying a digest of the frame it authenticated,
verified at chunk `k` by re-hashing the frame. What it would trust: that
return data persists across CPIs within one top-level instruction and is not
reset between top-level instructions, and that the ack chain binds the
current instruction index -- runtime properties nothing in this tree pins.
Even granting them, chunk `k` must still pay entry + decode + heap (17,050),
the top-level load (10,939), its own caller authority (6,792), a frame re-hash
no smaller than the transcript (26,059), and the family evaluation (100,064)
to have a bank to slice: at least 160,904 per extra chunk. On the commit's
1.2-1.3M single-invocation figure that is a coin flip against 1,399,700, and
it leaves Remove (3 chunks) and scenario (6) where they are. A hole in
principle and a loss in arithmetic; not pursued.

### (d) The bank itself

`project_candidate_bank` (`v4_equity_accelerator_accounts.rs:1153-1300`)
overwrites all 26 scalars and 36 of 37 identities; the AccountProfile authored
only the current slot, the Trading identity and the evidence owner
(`dealer/v3_profile.rs:186-233`). Trading holds the facts behind most of the
rest -- expiry and witness bytes from the request, the constant witness
offset, the parent digest it already computes (`hot_v3.rs:6959`), and every
frame key above -- so the honest semantic core is the four transition scalars,
the amounts and revisions per slot, and the content ids. The exact diet that
fits: dropping the 16 per-slot duplicates of release set, market, realm,
caller program, mint, token program, payer and rent refund leaves 26 scalars +
21 identities = 880 bytes. Zero bytes to spare, for Add only; Remove's 29
remaining identities are 1,208 bytes; General's scalars alone exceed the
payload. And every dropped register makes Trading a second author of a bank
whose whole admission claim is that the accelerator reproduces TransitionVM's
COMPLETE output (`v4_equity_accelerator_accounts.rs:1181-1184`,
`resolve_execution_candidate_v2`, `v2.rs:905-923`). Refuted as a channel.

### (e) What the tree already holds

- `ScratchPageKindV2::Candidate` (`v2.rs:1493-1512`), in the Lean-owned wire
  since the transport was written, documented as "Candidate AOT output
  assembled by Trading", constructed by nothing. The output-page idea has been
  reserved for since V2; only the ownership was left unsaid.
- The Shadow transport is digest-only by design: a 528-byte fixed ack
  (`shadow_v3.rs:38-41`), Trading interprets and the accelerator returns
  digests to compare (`shadow_v3.rs:1-8`). The tree already accepts "the
  authority is the digest in return data; the bytes travel elsewhere".
- Decision 0017's continuation carries a Registry receipt "as an ephemeral
  admission PDA signer instead of as CPI return data. That is a different
  transport for the same fact"
  (`docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md:241-242`).
- The INPUT scratch pages have no live producer. Outside tests they are
  written only by the ProgramTest bundle builder
  (`bundle.rs:838`) and as genesis accounts by the successor bootstrap
  (`family_hot_campaign.rs:634-645`, `write_genesis_account_v1`). No route,
  operator or browser writes a Trading-owned input page on a live chain. The
  General accelerator refuses inline input outright
  (`general-accelerator-sbf/src/lib.rs:603`). That is a second, separate wall
  for General on devnet and is out of this note's scope, but any design that
  reasons "pages are what Trading already does" is reasoning from fixtures.
- Decision 0003 anticipated exactly one accelerator posture -- "may not
  create a second state, claim, custody, or release authority" (`:12-16`) --
  and lists "wrong return-data producer" and "overwritten return data" among
  required refusals (`:262-263`, `:278`). Nothing in it forbids an owned
  scratch account; nothing in it admits one either.

## 3. Recommendation

Take (a) in its persistent, client-provisioned form: a third transport profile
whose accelerator writes the complete candidate bank into one writable account
it owns, passed at one fixed coordinate appended to the admitted CPI frame, and
acknowledges with the 144-byte header alone; Trading hashes the page and
requires the digest the producer-checked ack already carries. One caller
authority, one CPI, no loop, no System program in Hot, no close route on the
critical path. Every diet -- (b), (d), and their combinations -- fits at most
the equity Add and at zero margin while General's scalars alone overflow the
payload, and (c) buys back less than the chunk costs while resting on runtime
return-data semantics nothing here pins. The invariant weakens from "owns no
account" to "owns one scratch account that no route ever reads and no digest
ever fails to bind"; the census gets a stronger measurement out of it.

**First commit.** Contract only, inert until a Strategy record names it:
`ExecutionStrategyV2Abi.lean` gains the OutputPage request/ack schemas and a
third transport tag with width theorems; `EmitExecutionStrategyV2AbiRust.lean`
emits them into `generated_v2.rs`; `v2.rs` gains
`AcceleratorTransportProfileV2::OutputPageV3`, the two codecs, and hostile
decode tests including the null-payload ack and a V2 ack presented under the
V3 schema; `admitted_v3.rs` gains the appended page coordinate. Nothing
executable changes; no record re-digests until a Strategy selects the pair.
The pins test and the `v2_generator_fresh` gate carry it.

**Ruling needed from ember.** One: whether an admitted accelerator that owns
exactly one client-provisioned scratch page -- written only inside its CPI,
read only by Trading inside that window, never read by any route, bound by a
digest the runtime attributes to the writer -- is still the "stateless
accelerator" decision 0003 admits, or needs an amendment to 0003 saying so in
its own words. That is the only judgment call in the design; the rest is
measured or derived. The devnet cohort's Strategy/Certificate/Admission
records re-digest under the new pair, which strands nothing that AGENTS.md's
standing full-redeploy grant does not already abandon; rent reclamation for a
pooled page is a later accelerator Close route and is named here as debt.
