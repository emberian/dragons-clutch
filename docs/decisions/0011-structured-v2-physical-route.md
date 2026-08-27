# Decision 0011: the Structured V2 candidate is a host-side oracle, not a chain seam, and the frame is not an AccountProfile

Status: accepted on 2026-08-27 as the resolution of the STRUCTURED-PHYSICAL
charter's first item ("the capability route ... through the established
machinery"). This is a route and ownership decision. It is not release
evidence, it does not claim that any Structured action executes today, and it
deletes nothing: the two paths it supersedes are named in §5 with what each
still legitimately does.

## Context

Structured V2 shipped as a closed island (`M-9`): a kernel, a wire contract and
an operator that reference only each other, with no `programs/` file and no
census row. `5532dc5a` gave it derivations (`seeds.rs`) and a physical account
frame (`frame.rs`); `dee3311e` made the frame the sole author of the effect
account coordinates.

The charter this record answers described the remaining work as wiring the
island to the chain through a named seam:

> `StructuredHotCandidateInputV2` -> `prepare` -> `validate_token_poststate`
> per CPI -> `validate_root_poststate` commit-last

That seam does not exist, and this record is written because the next lane
would otherwise implement against it.

## 1. The candidate has no chain caller, and cannot acquire one

`crates/dclutch-structured-v2-contract/src/hot_v2.rs` (547 lines) describes
itself as the "onchain-safe execution candidate for common Trading Hot". Every
caller of `prepare`, `validate_token_poststate` and `validate_root_poststate`
in the tree is a test:

```text
crates/dclutch-structured-v2-contract/tests/hot_v2.rs
crates/dclutch-structured-v2-operator/tests/actions.rs
```

and `programs/dclutch-trading-sbf/Cargo.toml` does not depend on
`dclutch-structured-v2-contract` under any feature.

This is not a Structured oversight. `dclutch-fractional-claim-contract`'s
`FractionalHotCandidateV2` carries the identical three-method shape with the
identical zero non-test callers. Both belong to a family-side self-check
generation that predates the family-neutral dispatch decision.

**It cannot be wired.** For `hot_v3.rs` to call `prepare`, the Trading program
would have to link the Structured contract crate and run Structured-specific
Rust between Token CPIs. That is precisely the branch decision 0006 §3 rules
out ("`hot_v3.rs` gets no General branch"), and it would install a second
authority for facts the artifacts already own. A per-CPI family recheck is
also not a shape the executor has for anyone: `process_hot_execution_v3` is one
entry selected purely by the `DCLTHOT3` magic, and its only per-family
knowledge is the sealed artifact closure.

## 2. What the candidate legitimately is

A host-side oracle, and a good one. It hostile-decodes the request, rebinds it
to the terms, revalidates every amount against the immutable coefficients,
enforces the receipt-first-except-retirement ordering and the strictly
ascending shard sweep, and refuses aliased accounts. `tests/actions.rs` uses it
exactly this way: the operator plans an action, the candidate accepts or
refuses the plan, and that agreement is the evidence.

Its doc comment is what is wrong, not its content. It is the operator's
adversary, not the chain's.

## 3. The route Structured actually needs

The live seam is the sealed artifact closure, and it is entirely additive:

1. Four `CapabilityProgramV4` descriptors, one per `StructuredActionV2`
   (`Issue`, `Unwrap`, `TerminalRedeem`, `ZeroSupplyRetire`), each naming six
   artifacts by `(schema release id, sha256 of exact bytes)`.
2. One `CapabilityProgramSetV2` over them, `selector_offset` =
   `STRUCTURED_REQUEST_ACTION_OFFSET_V2`, `selector_width` = `U8`, selectors
   `0..3` strictly ascending, plus a fifth entry naming the ACTIVATION
   descriptor at a selector no request can produce (General uses 255).
3. A manifest `CapabilityEntryV1` whose `kind_id`, `release_id`, `config_id`,
   `child_schema_id` and `child_derivation_id` join the descriptor through
   `CapabilityProgramV4::validate_selection`.
4. A capability seal per `(descriptor, action)`, written by the already
   implemented `hot_v3::process_capability_seal_v1`.

Three constraints decide the artifacts, and all three are refusals that fire
before anything else is read:

- **The Effect program must be `EffectProgramV4`.** `decode_sealed_effect_v4`
  (`hot_v3.rs:7935`) refuses any other schema with `UnsupportedContent` as its
  first statement, and additionally refuses a nonzero `range_count`. This is
  the wall General is still behind. Because the effect digest feeds the
  descriptor, which feeds the seal, which feeds the ProgramSet identity,
  getting it wrong regenerates every artifact in the family.
- **The Transition must be `TransitionProgramV3`**, selected through an
  `ExecutionStrategyProgramV2` whose transition pair equals the descriptor's.
- **Declare zero dynamic fixed spans and `StrategyDispositionV2::Interpreted`.**
  A profile-only span forces `AdmittedAot`, which drags in eight authenticated
  strategy extras, per-page caller authorities and a deployed accelerator ELF.

**ERRATUM, same day, before any code was written against this section.** The
paragraph that stood here said Structured's six Token effect kinds "stop being
Rust and become `EffectProgramV4` operations resolved through the existing
`FixedRole` composition", and that "Structured has no Claims child of its own
... so it adds no new role". Those two clauses contradict each other, and the
first is false. Corrected in §3a; the rest of this section stands.

## 3a. There is no such thing as an effect operation that moves a token

An effect program cannot mint, burn, transfer or close a Token-2022 account.
The whole vocabulary is `ResolvedEffectV3` (`crates/dclutch-effect-kernel/src/v3.rs:390`):
`TransferLamports`, `WriteScalar`, `WriteIdentity`, `RequireLamportsEq`,
`WriteRequest` -- lamports, bytes into authenticated account data, and bytes
into a child route's request. No CPI, no invoke, no Token.

A family causes a Token CPI in exactly one way: its effect program declares a
**route** to a `FixedRole`, `WriteRequest`-patches that route's request
template, and the executor CPIs the release-selected program for that role,
which performs the Token CPI itself. So "resolved through the existing
`FixedRole` composition" *is* having a child; the erratum's two clauses could
never both be true.

`FixedRole` (`crates/dclutch-effect-kernel/src/v2.rs:354`) is closed at four --
`Core`, `Claims`, `Resolution`, `Custody` -- and only Claims can mint, burn or
close a Mint. Custody cannot substitute: `dclutch-custody-contract::OperationV1`
is `InitializeReplay | OpenVault | Transfer | CloseVault | CloseReplay`, with
no mint and no burn.

**So Structured needs a Claims child.** Its own §1 claim -- one backing edge,
stopping at the claim-shard layer -- describes its *economics*, not its
physics: the edge stops there, and moving atoms across it still requires the
role that owns them.

### The child ABI is a fork in the road, and it must be chosen before encoding

Trading refuses an unknown child request before the Claims program ever sees
it. `programs/dclutch-trading-sbf/src/claims_composition_v3.rs:332-449` is a
closed if/else chain over seven request magics ending in
`Err(TradingSbfError::Content)`. A `STRUCTURED_REQUEST_MAGIC_V2` child request
is refused there.

**Option A -- adopt the Rational child ABI. No new program code.** All six
kinds already execute on chain, and
`dclutch-rational-representation-v2-contract::TokenEffectStyleV2` names four of
them after Structured itself: `TransferShardToStructured`,
`TransferShardFromStructured`, `MintReceipt`, `BurnReceipt`, with closure via
`LifecycleActionV2::{RetireCoordinate, RetireReceipt}`. The multiply
`K_i = S · c_i` happens in the callee
(`rational-representation-v2-contract/src/plan.rs`), not in the effect program
-- there is no multiply opcode.

  The cost is identity. Structured would adopt Rational's authority PDA, and
  `seeds.rs`'s claim that the Structured root "is simultaneously the replay
  record, the receipt Mint authority and the shard custody owner" becomes dead
  -- along with `STRUCTURED_ROOT_PDA_SEED_V2` as a Mint authority. That is a
  larger rewrite of the island than the encoder is.

**Option B -- give Structured its own Claims child.** A new handler in
`programs/dclutch-claims-sbf`, plus a new magic arm and `ReceiptKindV3` variant
in `claims_composition_v3.rs` and its decoder. This keeps Structured's own
derivations. It is **not** a `hot_v3.rs` edit -- the four-role match is
untouched -- but it **is** a Trading-program edit, so §6's "no `hot_v3.rs`
change" is literally true and misleading in spirit.

This record did not choose. It recorded that the choice exists, that it
determines every byte of the effect program's route template, and that it must
therefore be made **before** the encoder is written, not during.

## 3b. AMENDMENT: Option A is taken, and the binding requirement it rides is SATISFIED

Amended by the `STRUCT-CHILD` lane on 2026-08-27, before any encoder byte was
written, per §6's instruction that "whoever writes the encoder amends §3a with
the option taken and the reason, first."

**Option A is taken.** Structured routes through the existing Rational child
ABI. Zero new program code in `programs/dclutch-trading-sbf` or
`programs/dclutch-claims-sbf`; no new request magic; no new `ReceiptKindV3`
variant; `hot_v3.rs` untouched under the four-role match, as §6 already said.

### The requirement that rode the choice

The choice was conditional on one thing: the adopted authority's seeds must
bind a per-family context, so that cross-family substitution refuses. If the
Rational authority's seed tuple could not express that without a **new seed
component**, the lane was to stop, because a seed addition is protocol work
needing its own ruling.

**Read the tuple.** The Rational representation authority -- the PDA that signs
every Token CPI in the route -- is derived at
`programs/dclutch-claims-sbf/src/rational_representation_v2.rs:486` and again
at `:1141`, and identically in the lifecycle route at
`programs/dclutch-claims-sbf/src/rational_lifecycle_v2.rs:376` and `:1145`:

```rust
Pubkey::find_program_address(
    &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &header.descriptor_id],
    program_id,          // the Claims program
)
```

Two components: one domain constant
(`b"dclutch:rational-authority:v2"`,
`crates/dclutch-rational-representation-v2-kernel/src/lib.rs:70`) and one
variable, `descriptor_id`. There is no context component, and there is no
unused one.

**`descriptor_id` is a sufficient per-family context, and it needs no new seed.**
It is the SHA-256 content identity of a `RepresentationDescriptorV2` record,
and the whole physical namespace of the route hangs off it. Every one of these
is re-derived inside the Claims program from `header.descriptor_id` and
compared against the supplied account, so naming descriptor `D` gives access to
`D`'s objects and to nothing else:

| resource | seeds | site |
|---|---|---|
| representation authority | `(domain, descriptor_id)` | `rational_representation_v2.rs:486` |
| shard Mint | `(RATIONAL_SHARD_MINT_SEED_V2, descriptor_id, outcome_le)` | `:975` |
| Structured shard custody Token account | `(RATIONAL_STRUCTURED_CUSTODY_SEED_V2, descriptor_id, outcome_le)` | `:988` |
| Claims custody owner | `ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor_id, outcome)` | `:310` |
| replay record | `(RATIONAL_REPLAY_SEED_V2, descriptor_id, actor)` | `:743` |

and the descriptor is itself admitted against exactly one Market:
`authenticate_rational_product_v3` (`rational_product_v3.rs:154-166`) refuses
unless `admission.market_id() == header.market`,
`admission.product_id() == market.product_instance_id`,
`admission.semantic_basis_id() == market.basis_id`,
`admission.receipt_mint() == header.receipt_mint` and
`admission.representation_authority() == header.representation_authority`,
with the Market aggregate itself pinned at
`(LIABILITY_BASIS_MARKET_SEED_V2, header.market)`.

So the hostile §3a was worried about -- an authority minted in one family's
context reaching into another family's shards -- **cannot be constructed**:
there is no authority that exists independently of a descriptor to bring.

**The evidence that no new seed component is needed is also the evidence that
none could be added cheaply.** The descriptor preimage
(`crates/dclutch-rational-representation-v2-kernel/src/lib.rs:740-782`) has
exactly ten reserved bytes -- six at `DESCRIPTOR_RESERVED_HEADER_OFFSET` and
four at `DESCRIPTOR_RESERVED_OFFSET` -- and `decode` runs `require_zero` over
both. A 32-byte Structured `terms_id` does not fit in ten bytes, and would be
refused at decode even if it did. Extending the tuple would have meant either a
new component under `RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2` (moving every
live Rational authority address) or `DESCRIPTOR_MAGIC_V3` -> V4 (moving every
descriptor_id, hence every shard Mint, custody account, Position and replay
record for every live representation). Both are exactly the class of work §3a
said needed its own ruling. Neither is required.

### What the ruling cost, stated in full -- it is more than §3a said

§3a said the cost was that Structured "would adopt Rational's authority PDA and
`seeds.rs`'s claim that the Structured root 'is simultaneously the replay
record, the receipt Mint authority and the shard custody owner' becomes dead".
That understated it in three ways, and the amendment records all three so the
identity rework is done against the truth rather than against §3a.

1. **All three Structured derivations lose their referent, not just the Mint
   authority.** `STRUCTURED_ROOT_PDA_SEED_V2`,
   `STRUCTURED_RECEIPT_MINT_PDA_SEED_V2` and
   `STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2` are all replaced by descriptor-keyed
   Rational derivations. `terms_id` -- the Structured root's identity --
   appears in no seed tuple, no descriptor field, and no request header field,
   and is read by no on-chain program. It survives as a host-side name for a
   `descriptor_id`, and `seeds.rs` must say so.

2. **Replay stops being a property of the node and becomes a property of the
   pair.** `seeds.rs` says the Structured root "is one per finalized terms
   record and carries no owner: replay is a property of the node, not of an
   actor." Rational's replay is `(RATIONAL_REPLAY_SEED_V2, descriptor_id,
   actor)` -- one record **per actor per representation**. That sentence is
   false under Option A. It is a semantics change, not a renaming.

3. **The receipt Mint authority is adopted in TWO Token-2022 roles, not one.**
   The representation authority is the Mint authority for `MintReceipt`
   (`mint_to_checked`, `:1239`) *and* the permissioned-burn authority for
   `BurnReceipt` (`permissioned_burn_instruction::burn_checked`, `:1362`,
   where the PDA is the burn authority and the ACTOR is the token owner).
   Founding must configure both roles on the receipt Mint, or `BurnReceipt`
   fails at the Token program with the descriptor already committed.

### Two divergences the encoder must absorb, found by reading the wire

Neither is a blocker; both would have been expensive to find during encoding.

**The six kinds are not six `TokenEffectStyleV2` kinds -- they are four plus
two on a different wire.** `StructuredHotTokenKindV2`
(`structured-v2-contract/src/hot_v2.rs:92`) and `TokenEffectStyleV2`
(`rational-representation-v2-contract/src/plan.rs:35`) map:

| Structured kind | Rational | wire |
|---|---|---|
| `MintReceipts` | `TokenEffectStyleV2::MintReceipt` | representation, `IssueStructured` |
| `BurnReceipts` | `TokenEffectStyleV2::BurnReceipt` | representation, `UnwrapStructured` |
| `LockShards` | `TokenEffectStyleV2::TransferShardToStructured` | representation, `IssueStructured` |
| `ReleaseShards` | `TokenEffectStyleV2::TransferShardFromStructured` | representation, `UnwrapStructured` |
| `CloseCustody` | `LifecycleActionV2::RetireCoordinate` | **lifecycle** |
| `CloseReceiptMint` | `LifecycleActionV2::RetireReceipt` | **lifecycle** |

The two closure kinds are not `TokenEffectStyleV2` members at all, which is the
mechanical reason §6's last bullet is right that `ZeroSupplyRetire` is a second
encoder. Conversely `TokenEffectStyleV2::{MintShard, BurnShard}` are
**unreachable from Structured**: they belong to `Denominate`/`Reconstitute`,
which create and destroy shards. Structured only ever moves shards that exist.

**`Issue`'s effect ordering INVERTS.**
`dclutch-structured-v2-operator::build_supply_effects` (`action.rs:308`) pushes
the receipt effect first and then sweeps the shards, for both Issue and Unwrap.
`TokenEffectIterV2` (`plan.rs:306`) emits:

- `IssueStructured`: `TransferShardToStructured` at cursors `0..K-1`, then
  `MintReceipt` at cursor `K` -- **shards first, receipt last**.
- `UnwrapStructured`: `BurnReceipt` at cursor `0`, then
  `TransferShardFromStructured` at cursors `1..K` -- receipt first.

So Unwrap agrees with Structured's plan order and **Issue is reversed**. The
ordering is not negotiable on the wire: the callee indexes the asset row from
the cursor (`asset_index = cursor` when issuing, `cursor - 1` when unwrapping,
`plan.rs:395`), so an out-of-order effect reads the wrong asset row. Structured
must re-order its Issue plan; §2's "receipt-first-except-retirement" rule
becomes "receipt-LAST for Issue, receipt-first for Unwrap". Note that the
strictly ascending shard sweep is preserved for free, because the cursor is the
row index.

Also confirmed against the code, because the encoder depends on it: the callee
derives every effect. `K_i = c_i * S` is `asset.coefficient.checked_mul(header.quantity)`
at `plan.rs:263`, and `TokenEffectIterV2` synthesises the whole effect
sequence from the request header and asset rows. The encoder emits a request
header plus `K` asset rows and nothing else -- no effect list crosses the wire.

### The executable ceiling is K = 3, it is hard, and it is a Product-width cap

§6 already said the nearest twin caps at `K = 3`. Two things sharpen that into
a campaign constraint, and both are refusals rather than guidance.

**`K` is the full Product outcome width, not the count of backed coordinates.**
`RepresentationRequestV2::validate` (`request-contract/src/request.rs:470-481`)
refuses `IssueStructured`/`UnwrapStructured` unless
`asset_count == outcome_count` exactly, with `selected_outcome == u32::MAX`;
only the selected-outcome actions take `asset_count == 1`. Structured's own
model sweeps only nonzero-coefficient coordinates. On this wire every outcome
needs a full asset row **and its materialized account quadruple** (shard Mint,
actor token account, custody token account, Claims Position) even at
coefficient zero, where `prepare` computes `effect_amount = 0` and a
zero-amount `transfer_checked` still executes and still costs CU.

**The bound is the RequestProfile artifact, and `K = 4` is refused with 8 bytes
to spare at `K = 3`.** `REQUEST_PROFILE_MAX_BYTES_V1 = 1312`
(`crates/dclutch-request-profile-contract/src/generated.rs:4`) bounds one
serialized RequestProfile V1: `32 + ops * 24`, enforced at
`request-profile-contract/src/encode.rs:253` and `src/lib.rs:195`. So
`ops <= (1312 - 32) / 24 = 53`. The canonical projection of a
`RepresentationRequestV2` is `29 + 8K` operations
(`open_structured_v3.rs:131-132`, asserted at `:674-678`; the 29 decomposes as
13 `require_*` + 9 `project_identity` + 5 `project_u64` + 2 `project_u32`).
Therefore `K = 3` gives 53 operations and 1,304 bytes -- **8 bytes of slack,
which is not a fourth operation (24 bytes)** -- and `K = 4` gives 61 operations
and 1,496 bytes, refused at `encode.rs:253`. Structured projects the identical
request layout, so it inherits the identical 29, and the ceiling is not
negotiable by encoding more cleverly.

Note the ceiling belongs to the RequestProfile and **not** to the effect
program: `encode_effect` emits `17 + 8K` operations
(`open_structured_v3.rs:135-136`, asserted at `:921-925`) and no byte bound on
`EffectProgramV4` exists in `dclutch-effect-kernel`. The AccountProfile is not
binding either -- `48 + 16*(37 + 4K) + 16` admits `K = 10` under the same 1,312.
Only the RequestProfile bites.

**So: Structured on the Rational wire is limited to Products of outcome width
`N <= 3`**, and `STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 = 257` remains what §6
called it -- a capacity-profile measurement with no executable meaning.

Widths for anyone sizing artifacts: request `488 + 160K`
(`request-contract/src/generated.rs:3-4`), so 968 bytes at `K = 3`; effect
program `32 + 32 + 24*(17 + 8K) + 488 + 160K`, wrapped in a 24-byte V4
envelope, so 2,040 bytes at `K = 3`; lifecycle request
`400 + 272 * coordinate_count`
(`lifecycle-contract/src/lib.rs:35,37`, asserted at `:497-503`), which carries
no cap of its own and is bounded only by its own caller's profile.

## 3c. AMENDMENT: Option A means Structured authors NO artifacts. They are all already landed.

Same lane, same day, found while sizing the campaign. This is the largest
consequence of the ruling and §3 does not contain it.

§6 said the nearest twin
`dclutch-bearer-v2-operator::open_structured_v3::encode_effect` "settles the
mechanical shape of the encoder". Read again with Option A taken, that
understates it to the point of being misleading. The twin is not a shape
reference. **It is the artifact.**

`crates/dclutch-bearer-v2-operator/src/open_structured_v3.rs:1` describes
itself as "Data-defined Hot artifacts for full-width Structured issue and
unwrap", and `build_rational_open_structured_hot_bundle_v3` (`:188`) takes a
`RepresentationDescriptorV2` plus a `RepresentationActionV2` and returns the
complete bundle: `account_profile`, `request_profile`, `lifecycle_policy`,
`transition`, `strategy`, `effect`, and the `CapabilityProgramV4` descriptor
selecting all of them. It is parameterized by the descriptor, so it serves any
representation -- including one founded from Structured terms.

The other two actions are in the same position:

| Structured action | Rational action | already-landed builder |
|---|---|---|
| `Issue` | `IssueStructured` | `build_rational_open_structured_hot_bundle_v3` (`bearer-v2-operator/src/open_structured_v3.rs:188`) |
| `Unwrap` | `UnwrapStructured` | same builder |
| `TerminalRedeem` | `UnwrapStructured` then `RedeemTerminal` | same builder, then `build_rational_terminal_hot_bundle_v3` (`bearer-v2-operator/src/hot_bundle_v3.rs:96`) |
| `ZeroSupplyRetire` | `RetireCoordinate` | `build_rational_lifecycle_hot_bundle_v3` (`rational-lifecycle-hot-v3/src/bundle.rs:94`) |
| `ZeroSupplyRetire` | `RetireReceipt` | the compact V4 path -- `bundle.rs:92` states complete-support receipt retirement is "exclusively compact V4", and `artifacts.rs:228` refuses `RetireReceipt` in V3 |

**So §3's numbered artifact chain, and §6's "the order of work is fixed by the
digest chain: EffectProgramV4 first, then the AccountProfile ... then the
descriptor, then the set", is already executed for every Structured action.**
The warning that "an effect-schema mistake invalidates everything downstream of
it" is no longer a risk this family carries, because this family is not
choosing an effect schema. It is reusing four that are digest-stable and
landed.

None of these builders has a caller outside its own crate today
(`open_structured_v3` is re-exported at `bearer-v2-operator/src/lib.rs:75` and
otherwise unreferenced), which is why the twin read as a parallel effort rather
than as Structured's own route.

### What actually remains, then

1. **Derive a `RepresentationDescriptorV2` from the Structured terms.** This is
   the one genuinely new host-side artifact and it is small: the descriptor
   preimage is a 10-byte-reserved header plus `graph_id`, `graph_digest`,
   `root_id`, `market_id`, `release_set_id`, `receipt_mint`, `token_program`,
   `outcome_count`, `denominator` and `K` coefficients, every one of which the
   Structured terms already carry or determine. Its digest is `descriptor_id`,
   and §3b showed that identity is the whole of Structured's on-chain physics.
2. **Feed it to the four builders**, once per action, and seal per
   `(descriptor, action)` with the already-implemented
   `hot_v3::process_capability_seal_v1`.
3. **The `CapabilityProgramSetV2` and the manifest entry** (§3 items 2 and 3),
   which are joins over descriptors that now exist rather than artifacts to
   author.
4. **The campaign and the census bindings** (§6): a
   `tools/gauntlet/structured/` directory whose `bindings.json` binds to
   existing trading routes. Still no `TARGETS` row; Structured still ships no
   `programs/` entrypoint.

The lowering from Structured's own plan onto that wire is landed in
`crates/dclutch-structured-v2-operator/src/child_request.rs` with per-kind
witnesses (16 tests), including the three divergences §3b records.

### What Fractional's twin inherits

All of it, and this is the reusable part. §5 said Fractional's physical twin
"needs this record before it needs code". Sharper now: if Fractional takes
Option A, it authors no artifacts either -- its work is a descriptor
derivation, a kind-to-style lowering with its own divergence audit, and a
campaign. The generalisation is that **a family adopting an existing child ABI
inherits that ABI's whole artifact closure**, and the expensive part is not the
encoder but discovering where its own semantics disagree with the wire's.

## 3d. CORRECTION: "already landed" was true of the code and false of its behaviour

STRUCT-CAMP, 2026-08-27, found while taking §3c's short path. Two corrections
to §3c and one to §3b, all measured.

### The builders did not run

§3c's central claim is that Structured authors no artifacts because the four
builders are "already landed". They are written. At the moment §3c was
recorded, `dclutch-bearer-v2-operator` was **5 of 20** and every profile-emitting
test in it had been failing since 2026-08-26, from two sweeps that each stopped
one producer short:

- `cc228cdd` made a nonzero privilege on an `AuthenticatedRouteAlias` a refusal.
  The rule is right — `authenticate` takes `representative_privileges` for any
  coordinate whose representative is another (`v2.rs:2360-2369`) and never reads
  the alias's own field. WAVE.md records that it "silently broke every Profile14
  emission — fixed producer-side"; that fix reached the Direct producer and not
  the three in this crate, all of which marked their Claims/Token-program
  placeholder aliases `executable`.
- `ca5e5f14` moved `HOT_FIXED_ACCOUNT_COUNT_V3` from 38 to 39, and three
  transaction tests kept the old account indices. Invisible beneath the first
  break, which panicked in the shared fixture before any assertion ran.

Both are fixed (`57c8fc3c`, 21/21). **The generalisable rule, which belongs in
any future ruling of this shape: an artifact builder with no caller outside its
own crate is not landed, it is parked.** A builder with no callers has no gate,
so "already landed" should be read as "already written" until something outside
the crate drives it. `b99d6adf` gives these four their first such caller, and
the descriptor it hands them is the first one they have ever been given that a
Record account could hold: the crate's own fixture hand-fills the preimage and
then supplies an arbitrary `descriptor_id` that is not the digest of those
bytes, while the Claims adapter computes `descriptor_id = hash(record data)`.

### `graph_id` is the exposure bundle, and §3b's lowering joined the wrong one

`RepresentationDescriptorV2::graph_id()` / `graph_digest()` name the
`CompositionExposureBundleV3` record, not the source graph: the adapter hands
them to that decoder as `RecordAdmissionV3` under
`COMPOSITION_EXPOSURE_SCHEMA_ID_V3`
(`rational-representation-v2-operator/src/lib.rs:558-576`) and
`authenticate_exposure` requires equality with `exposure.bundle_id()`
(`rational-representation-v2-kernel/src/lib.rs:902`). The descriptor's own
encoder already names the field `exposure_id`; the decoder's accessor is the
legacy name, and it is what made the first version of §3b's lowering compare
`StructuredTermsV2::graph_id` — a record the terms decoder *proves* is a
different one, so the join could never be satisfied by a descriptor the chain
would accept. Fixed at `55378ca6`; the field is now `exposure_id` in Structured's
own type, because the shared name is the defect.

**Fractional's twin inherits this trap specifically**, on top of §3c's
generalisation: any family lowering onto this wire will read the name `graph_id`
and join its own graph identity.

### The live route no longer checks that the coefficients are the recipe

`authenticate_graph` held `coefficient * scale == root_exposure * denominator`
per outcome. It reads the superseded `RepresentationGraphV2` and has zero
non-test callers; the live route runs `authenticate_exposure`, which checks the
bundle's identity, digest and width and never the coefficients. Combined with
§3b's finding that the Structured terms reach no on-chain reader at all, that
makes **founding the last moment the recipe can be checked against the
composition it claims to represent**. It is tolerable — the coefficients are
immutable and the descriptor is content-addressed, so a wrong recipe is a wrong
founding rather than a forgeable request — but it must be checked somewhere, and
`derive_structured_representation_descriptor_v2` is where.

Recorded as a **RECORDS-MIGRATE** row rather than resolved here: `root_id` has
no live consumer at all, and `graph_id`/`graph_digest` are double-booked between
the dead legacy-graph path and the live exposure path. Collapsing that moves
every live `descriptor_id`, hence every shard Mint, custody account, Position and
replay record of every representation.

### The campaign is a lane, not a project

`programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`
already executes `IssueStructured`, `UnwrapStructured`, `Denominate`,
`Reconstitute` and terminal redemption on real Claims/Custody/Registry/Core ELFs
with a genuine Token-2022 v11, with 28 census bindings at
`tools/gauntlet/claims-rational-representation-v2/`, at `K = 2`, coefficients
`[3, 7]`, denominator `10`. Its descriptor is planted by `add_finalized_record`,
so the on-chain route already accepts a self-identifying descriptor. Structured's
campaign is that test parameterized by
`derive_structured_representation_descriptor_v2` at `K = 3`, plus the
family-specific hostiles and a `tools/gauntlet/structured/` binding directory.
It needs no `DCLTGMF1`, no successor bootstrap and no validator.

## 4. `frame.rs` is two objects, and only one of them is an artifact input

`frame.rs` is now load-bearing for the effect coordinates (`dee3311e`), and
that half is correct: the operator and the candidate had two disagreeing
account layouts, and the frame is the one author they now share.

Its 23-account BASE is a different object, and it is not an AccountProfile
expansion. The Trading hot frame already carries 39 fixed accounts, and
`expand_runtime_accounts_v3` (`hot_v3.rs:4120`) injects five more at profile
coordinates 0..4 -- root, config, product, portfolio, linked basis -- before
the family's supplied suffix begins at coordinate 5. Against that, of the
frame's 23 base coordinates:

| frame coordinate | already owned by |
|---|---|
| `ROOT` | injected at runtime coordinate 0 |
| `CORE_MARKET` | hot fixed 0 |
| `CORE_PROGRAM` / `CORE_PROGRAMDATA` | hot fixed 23 / 24 |
| `REGISTRY_PROGRAM` | hot fixed 27 |
| `ACTIVATION_CACHE` | hot fixed 22 |
| `RENT_SYSVAR` | hot fixed 28 |
| `CLAIMS_PROGRAM` / `CLAIMS_PROGRAMDATA` / `RENT_PROGRAM` | resolved per child route by the composition, not by the profile |
| `CALLER_AUTHORITY` / `CALLER_PROGRAM` / `CALLER_PROGRAMDATA` | derived per child route by the executor; absent entirely for an interpreted strategy |

Thirteen of twenty-three. The ten that are genuinely family-runtime accounts
are `ACTOR`, the two terms Record pairs, `TOKEN_PROGRAM`, `RECEIPT_MINT`,
`RECEIPT_TOKEN`, `RENT_CREDIT` and `SYSTEM_PROGRAM`, plus the per-coordinate
triples.

`frame.rs` was authored as a standalone instruction frame -- the account list a
Structured program with its own entrypoint would parse. Structured has no such
program and is not getting one. **Transcribing the base into an
`AccountProfileV2` would install a second authority for seven accounts the hot
frame already fixes**, which is decision 0006 §2's objection restated in a
family crate instead of the executor.

The AccountProfile emitter this family still needs must therefore be authored
against the runtime coordinate space (family accounts from coordinate 5), and
`frame.rs`'s base becomes either its input for the ten family coordinates only,
or a test-only cross-check. The per-coordinate triple stride and the effect
slot assignment carry over unchanged; they were never in dispute.

## 5. What this record supersedes, and what it does not delete

- `crates/dclutch-structured-v2-contract/src/hot_v2.rs` is **not** deleted. It
  is retargeted in intent: a host-side oracle for the operator, whose module
  documentation should stop claiming a chain role. It is the only adversary the
  operator's plans currently have.
- `crates/dclutch-fractional-claim-contract/src/hot_v2.rs` is in exactly the
  same position and should be read the same way. Fractional's physical twin
  needs this record before it needs code.
- `frame.rs` keeps its effect-coordinate authority and loses its implied claim
  to be the account frame the chain expands.

## 6. Consequences

- No `hot_v3.rs` change is required or permitted by this route, under either
  child option: the four-role dispatch is untouched. **Option B does require a
  Trading-program edit** in `claims_composition_v3.rs`, which is a different
  file and a different owner, and this bullet originally read as though it
  ruled that out. The coordination stop the charter reserved against `DECOMP-r`
  is still not needed.
- Structured adds no row to the census `TARGETS` table
  (`tools/gauntlet/census/src/main.rs:41`), because that table lists
  `programs/` entrypoints and Structured ships none. It appears in the census
  by acquiring a `tools/gauntlet/structured/` campaign directory whose
  `bindings.json` binds its transactions to existing trading routes, with
  per-transaction `logs` so `census observe` can cross-check them.
- The cheapest executable width is small. The nearest structural twin,
  `dclutch-bearer-v2-operator::open_structured_v3`, caps at `K = 3`
  (`RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3`) because RequestProfile
  V1's 1,312-byte bound admits 29 prefix plus 8 per-row operations.
  `STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 = 257` is a measured capacity-profile
  bound, not an executable one, and no campaign should be sized against it.
- The order of work is fixed by the digest chain: EffectProgramV4 first, then
  the AccountProfile against the runtime coordinate space, then the remaining
  four artifacts, then the descriptor, then the set, then the campaign. An
  effect-schema mistake invalidates everything downstream of it.

  **The §3a child-ABI choice sits ahead of even that.** It decides the route
  template the effect program patches, so choosing it late invalidates the
  first item in the chain and therefore all of them. Whoever writes the encoder
  amends §3a with the option taken and the reason, first. **Done: §3b, Option
  A, 2026-08-27.** The chain below is now unblocked, and the route template it
  patches is a `RepresentationRequestV2` (representation wire) or a
  `LifecycleRequestV2` (retirement wire).

- The mechanical shape of the encoder, once the ABI is chosen, is settled by
  the nearest twin (`dclutch-bearer-v2-operator::open_structured_v3::encode_effect`):
  one route, `FixedRole::Claims`, `RouteKindV3::Once`, `span_count = 0`,
  `range_count = 0`, `BorrowedRangePolicyV4::DisjointExactCoverage`,
  `semantic_prefix_bytes` = the child request width, `item_*_stride = 0`
  throughout, and K flattened into COMMON registers so the artifact is
  regenerated per K rather than parameterised by it. `ProgramV4` is a 24-byte
  envelope over a V3 body; the encode is two steps,
  `encode_effect_program_v3_atomic` then `v4::encode_program_v4_atomic`.
  Note that `dclutch_effect_kernel::v3::encode::encode_effect_program_v4_atomic`
  is NOT a V4 encoder -- the "v4" there names the plural receipt-dependency
  table revision and it emits a V3 body.

- `ZeroSupplyRetire` needs a different child ABI from the other three
  (lifecycle, not representation), so it is a second encoder rather than a
  fourth branch of the first.
