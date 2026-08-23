# Product and Series pure core

`clutch-product-series` is the allocation-free semantic and transition core for
recurring Dragon's Clutch products. It freezes typed content identities, exact
Product/Series joins, absolute window and evidence-only repair arithmetic,
authenticated-adapter seams, canonical SourcePlane V3 occurrence provenance,
and component-by-component funding transitions. It has no account tags,
instruction intents, Solana SDK, Token-2022, oracle SDK, CPI, account memory,
allocator, floats, or caller-selected market nonce.

## Exhaustive quantized interval consensus

`QuantizedIntervalConsensusWorkV1` is the fixed 592-byte, allocation-free work
contract for lifting smooth point-only evidence without guessing a point. It
binds the full-width Market V2, Product template, Genesis, native basis,
SourceOccurrence, immutable SourcePlane interval result, price-measure policy,
capability profile, evaluator release, and canonical `WEIGHT-ROUND-01` identity.
Each bounded advance evaluates the next integer coordinate with the Product
basis evaluator, latches the first exact payout vector, and refuses immediately
if any later vector differs. A certificate exists only after the inclusive
upper endpoint has been evaluated.

The fixed work codec is structural and non-authorizing. The pure in-memory
session can mint a private verified payout capability because its history starts
at the checked Begin constructor. Restoring that capability from a persisted
work record is intentionally absent, and
`require_quantized_interval_consensus_runtime_capability_v1` always refuses.
A future SBF integration must authenticate the dedicated work PDA, owner,
lifecycle, transcript succession, and Replay transition before a Failure
relation successor may consume the certificate and install a resolution.

The crucial identity split is:

```text
MarketInstanceId = H(TemplateId, MarketGenesisProfileId, start, cap)

PriceMeasurePolicyV1Id = H(exact quantized checker and bound contract)
MarketInstanceV2Id   = H(TemplateId, MarketGenesisProfileV2Id, start, cap)
SeriesPlanV5Id       = H(TemplateId, GenesisProfileV2Id, AttachmentPlanId,
                          finite recurrence, cap)

SeriesFundingQuoteId   = H(exact per-component lamport/collateral amounts)
SeriesAttachmentPlanId = H(FundingQuoteId, LiquidityPlanId, WrapperSetId)
SeriesPlanId           = H(TemplateId, GenesisProfileId, AttachmentPlanId,
                           finite recurrence, cap)
SeriesFundingTermsId   = H(SeriesPlanId, refund/sink/mint/token identities)
```

Work price, liquidity, wrappers, funding sponsor, and refund destinations do
not fork an economically identical market. Changing the Template, immutable
Realm/profile venue semantics, absolute start, or collateral cap does.

## SBF publication boundary

The `clutch-sbf` adapter has an explicitly non-production
`non-production-product-series-lab` profile that can publish each canonical
body from this crate as a program-owned, content-addressed artifact. The
adapter decodes the hostile body with this crate's codec and checks the same
typed SHA-256 identity before sealing it. That route is an immutable catalog,
not a Series registry, funding state, occurrence compiler, Market creator, or
runtime price-witness activation path. In particular, freely constructible
registry and fulfillment projections in this pure core never become onchain
authority merely because their bodies can be published.

## Canonical rules

All integers are little-endian. Decode requires the exact named byte length;
reserved bytes are zero; fixed arrays use an active prefix and exact canonical
padding; required identities are nonzero. A typed identity is:

```text
SHA256(exact ASCII domain || exact canonical body)
```

There is no delimiter byte and no length prefix because both the domain and
body width are frozen by the typed codec. Registry-owned semantic values remain
opaque nonzero inputs. This crate allocates no global registry values.

### `NativeClaimBasisV1` — 2,352 bytes

Domain: `dragons-clutch/native-claim-basis/v1`

| Range | Field |
|---:|---|
| `0..8` | `DCBASIS1` |
| `8..10` | schema `1` |
| `10..18` | degree, outcome count, payout count, knot count, spacing, ambiguity, edge, zero flags |
| `18..24` | reserved zero |
| `24..32` | common denominator |
| `32..2080` | `[16][16]u64` payout weights |
| `2080..2096` | degree-zero payout map / `0xff` unused entries |
| `2096..2352` | `[16]u128` knots |

This artifact is the sole partition/basis/payout owner. It contains no failure
policy, failure payout index, or privileged payout vector. Every active payout
row is merely one vector the authenticated evidence relation may select.
Degree zero requires the spacing sentinel, assigns distinct finite payout rows
by first use while scanning ordered cells, references every active row, and
refuses duplicate rows. Degrees one through three derive their resolution
vector from the smooth basis and therefore require `payout_count = 0`, zero
payout rows, and an entirely unused payout map. A legacy kernel anchor row is
adapter baggage and does not enter this economic identity. Degree one's spacing
field is the exact log2 gap only for uniformly power-of-two-spaced knots and the
sentinel otherwise; degrees two and three require their exact uniform exponent.

### `EvidenceOnlyRecoveryPolicyV1` — 208 bytes

Domain: `dragons-clutch/evidence-only-recovery-policy/v1`

| Range | Field |
|---:|---|
| `0..8` | `DCRECV1\0` |
| `8..10` | schema `1` |
| `10..11` | attempt count |
| `11..16` | zero flags/reserved |
| `16..208` | eight 24-byte attempts |

Each attempt is generation delta `u64`, open offset `u64`, and exclusive close
offset `u64`, all relative to primary maturity. Active attempts are finite,
strictly generation-increasing, ordered, and non-overlapping. The raw economic
window never shifts; later attempts change authenticated repair generation and
deadline only. Exhaustion can lead to dormant recovery, never a numeric payout.

### `ProductTemplateV4` — 256 bytes

Domain: `dragons-clutch/product-template/v4`

| Range | Field |
|---:|---|
| `0..8` | `DCTMPLV4` |
| `8..16` | schema, statistic, coverage policy, zero flags |
| `16..48` | SourcePlane contract/release ID |
| `48..80` | SourceSpec ID |
| `80..112` | SummaryProgram ID |
| `112..144` | NativeClaimBasis ID |
| `144..176` | EvidenceOnlyRecoveryPolicy ID |
| `176..208` | compiler/relation release ID |
| `208..216` | primary window span |
| `216..224` | primary maturity grace |
| `224..232` | base repair generation |
| `232..240` | coverage parameter |
| `240..256` | reserved zero |

### `MarketGenesisProfileV1` — 352 bytes

Domain: `dragons-clutch/market-genesis-profile/v1`

| Range | Field |
|---:|---|
| `0..8` | `DCMGPV1\0` |
| `8..12` | schema and registry-owned terminal disposition |
| `12..16` | reserved zero |
| `16..336` | Realm, Profile, PriceGrid, fee, relation, score, candidate lifecycle, candidate liveness, retirement, capability-profile IDs |
| `336..344` | native bearer lot |
| `344..352` | reserved zero |

The live join supplies a complete market-core
`RegistryCapabilityProjectionV1`. The pure
core equality-checks its exact statistic, coverage, ambiguity, edge, and `BURN`
values; capability-profile and semantic-owner identities; degree and parameter
support; operational limits; and immutable Realm/Profile collateral facts. The
core also requires the basis denominator to divide the bearer lot. It allocates
no registry values. The projection is deliberately not a codec and cannot
authenticate itself: the adapter must derive it from an authenticated central
release manifest and immutable Realm/Profile account before live activation.

`MarketGenesisProfileV1` is frozen and remains decodable, but it does not own a
price-measure policy. It is therefore not eligible for a RelationV2 profile
that requires price coherence. The omission is repaired only by the successor
types below; no V1 byte or typed ID is reinterpreted.

## Quantized price-policy successor

### `PriceMeasurePolicyV1` — 96 bytes

Domain: `dragons-clutch/price-measure-policy/v1`

| Range | Field |
|---:|---|
| `0..8` | `DCPMPV1\0` |
| `8..10` | schema `1` |
| `10..16` | reserved zero |
| `16..48` | exact reviewed checker-release ID |
| `48..54` | checker version, quantized-semantics version, degree range, outcome/atom bounds |
| `54..64` | reserved zero |
| `64..88` | maximum payout denominator, witness denominator, and price scale |
| `88..96` | reserved zero |

This first policy schema admits only price-measure checker version 3 and
quantized-semantics version 1. That single version selects upstream exact
integer-simplex prices, the production integer-coordinate evaluator, and
largest-remainder/lowest-index payout quantization; callers cannot choose
alternate basis or rounding enums. Its exact Product domain is degrees zero
through three. Degree zero keeps the complete canonical Product finite payout
table: ordered cells may repeat a payout-row mapping, payout rows may be
non-one-hot, and payout count may differ from native outcome count. Degrees one
through three use the exact smooth B-spline body. The policy also freezes
maximum outcome/atom width, immutable payout denominator, primitive witness
denominator, and price scale.

The pure join checks the exact `NativeClaimBasisV1` body against those bounds.
A PriceGrid is a scalar tick lattice: it has a tick count and price scale, not
an outcome width. `validate_candidate_price_contract` instead requires the
candidate `PriceVectorV3` width and degree to equal the native basis, checks its
exact integer simplex and canonical padding, and requires its scale to equal an
adapter-authenticated PriceGrid scale. The venue adapter remains responsible
for proving active component membership in the authenticated tick lattice.
`validate_witness_contract` makes the policy's atom and primitive-denominator
bounds effective before the full V3 arithmetic check.

`NativeClaimBasisV1` is the sole persisted owner of the payout body and its
ambiguity/edge registry selectors. `MarketGenesisProfileV2` is the sole owner
of the closed coordinate bounds. The registry owns the selector-to-semantics
mapping; an adapter must authenticate that mapping before passing the resolved
edge enum to `project_smooth_basis`. The degree-zero and smooth helpers combine
those already-authenticated bodies ephemerally. Price-measure `basis_digest`
equals the exact `NativeClaimBasisV1Id`; a Relation/EconomicDomain digest joins
the Market/Genesis identity and its per-Epoch facts but is not free to choose
alternate bounds. Until an adapter authenticates those joins, runtime
price-witness activation remains deliberately blocked. Continuous or
unquantized semantics cannot be smuggled through this codec: they require a
distinct typed policy body and a new Genesis successor, and
`MarketGenesisProfileV2` will not coerce that future ID into its V1 policy
field.

For a smooth basis with registry-resolved `Refuse`, the Genesis coordinate
range must equal the inclusive first-to-last stored-knot span. Otherwise a
valid market coordinate could have no payout and the partition would not be
exhaustive. A registry-resolved `Clamp` may use a wider Genesis range because
both exterior intervals deterministically map to their nearest endpoint.

### `MarketGenesisProfileV2` — 416 bytes

Domain: `dragons-clutch/market-genesis-profile/v2`

This fresh `DCMGPV2\0`, schema-2 body preserves every V1 field, adds the exact
typed `PriceMeasurePolicyV1Id` after `PriceGridId`, and appends the exact closed
`u128` coordinate minimum and maximum after the bearer lot. Its identity
therefore changes when the checker release, quantized semantics, admitted
bounds, or market coordinate domain changes. The coordinate range must be
nonempty and contain every active basis knot; the degree-zero first boundary is
strictly above the lower bound. `CapabilitySemanticOwnersV2` and
`RegistryCapabilityProjectionV2` equality-check that same policy ID and exact
policy body in the complete registry join.

| Range | Field |
|---:|---|
| `0..16` | fresh magic, schema 2, terminal-disposition selector, reserved zero |
| `16..368` | eleven exact 32-byte identities, including PriceMeasurePolicyV1 |
| `368..376` | native bearer lot |
| `376..392` | coordinate-domain minimum |
| `392..408` | coordinate-domain maximum |
| `408..416` | reserved zero |

### Successor identity cascade

| Type | Bytes and offsets | Domain |
|---|---|---|
| `MarketInstancePreimageV2` | 88: magic `DCMKTIN2` `0..8`, Template ID `8..40`, GenesisProfileV2 ID `40..72`, start `72..80`, cap `80..88` | `dragons-clutch/market-instance/v2` |
| `SeriesPlanV5` | 152: fresh 16-byte schema-2 header, Template/GenesisV2/Attachment IDs `16..112`, recurrence/cap `112..152` | `dragons-clutch/series-plan/v5` |
| `SeriesFundingTermsV2` | 240: fresh 16-byte schema-2 header, SeriesPlanV5 ID `16..48`, lamport/collateral refund identities `48..112`, collateral-neutral token account `112..144`, lamport-neutral System account `144..176`, mint `176..208`, token program `208..240` | `dragons-clutch/series-funding-terms/v2` |
| `CompiledProductSeriesBundleV1` | 528: 16-byte header followed by sixteen exact identities: Registry release/profile, Source release/contract/spec/summary, compiler release, native basis+payout, recovery, Template, price approximation, Genesis, Quote, Attachment, Series, and FundingTerms | `dragons-clutch/compiled-product-series-bundle/v1` |
| `CompiledSourceOccurrenceV3` | 184: 16-byte header, SeriesPlanV5 ID + ordinal `16..56`, MarketInstanceV2/Attachment/Window/Statistic IDs `56..184` | `dragons-clutch/source-occurrence-record/v1` |
| `SeriesFundingStateV1` | 324: 16-byte header, Series/FundingTerms/Quote IDs `16..112`, cursor/lapse fields `112..124`, five 40-byte principal/donation/consumption compartments `124..324` | mutable state; no content ID |

`compile_ordinal_v2` returns `CompiledOrdinalV2` and a full
`MarketInstanceV2Id`. `project_component_debits_v2` reuses the authoritative V1
FundingQuote and Attachment amount bodies, because neither contains a V1
Genesis, Market, or Series ID, while requiring the fresh typed market ID in its
explicitly untrusted `AdapterFulfillmentProjectionV2`. Its public fields carry
no authentication authority: a live adapter must populate them only after
checking exact component accounts and capitalization.
`ProjectedComponentPresenceV2::ClaimedPresentExactAndCapitalized` is freely
constructible and describes only the projection's claim. Funding ownership uses
`SeriesFundingTermsV2`, because the persisted Series ID changed.

The successor market identity transitively commits the exact basis through
`ProductTemplateId`, fee and price policy through `MarketGenesisProfileV2Id`,
and start/cap directly. Series identity and operational attachments remain
separate on purpose so two Series can converge on one economic market. A live
successor Market account must nevertheless persist the full 32-byte
`MarketInstanceV2Id`; lowering it to a legacy 64-bit nonce is not an injective
identity bridge.

### Remaining exact bodies

| Type | Bytes and offsets | Domain |
|---|---|---|
| `MarketInstancePreimageV1` | 88: magic `0..8`, Template ID `8..40`, GenesisProfile ID `40..72`, start `72..80`, cap `80..88` | `dragons-clutch/market-instance/v1` |
| `SeriesFundingQuoteV1` | 280: header/count `0..16`, RecoveryPolicy ID `16..48`, five ordered `(lamports u64, collateral atoms u64)` components `48..128`, failure-root and permanent replay-tombstone rent principal `128..144`, Recovery rent principal `144..152`, eight `(progress cap u64, lamports/unit u64)` rows `152..280` | `dragons-clutch/series-funding-quote/v1` |
| `SeriesAttachmentPlanV1` | 112: 16-byte header, FundingQuote ID `16..48`, LiquidityFacilityPlan ID `48..80`, WrapperRecipeSet ID `80..112` | `dragons-clutch/series-attachment-plan/v1` |
| `SeriesPlanV4` | 152: 16-byte header, Template/Genesis/Attachment IDs `16..112`, first start `112..120`, stride `120..128`, count `128..132`, reserved `132..136`, lead `136..144`, cap `144..152` | `dragons-clutch/series-plan/v4` |
| `SeriesFundingTermsV1` | 208: 16-byte header, Series ID `16..48`, lamport refund `48..80`, collateral refund token account `80..112`, neutral sink `112..144`, collateral mint `144..176`, token program `176..208` | `dragons-clutch/series-funding-terms/v1` |
| `RegistryProgramReleaseV1` | 160: header `0..16`, Program/ProgramData/full-ProgramData SHA-256 `16..112`, deployment slot `112..120`, compiled capability-manifest ID `120..152`, reserved `152..160`; the release ID is derived from the complete body | `dragons-clutch/registry-program-release/v1` |
| `RegistryCapabilityProfileV2` | 800: header `0..16`, exact RegistryRelease ID `16..48`, selector mappings and hard limits `48..96`, fourteen semantic-owner IDs `96..544`, immutable Realm collateral `544..744`, exact SummaryProgram body `744..800`; the capability-profile ID is derived from the complete body and is not stored inside it | `dragons-clutch/registry-capability-profile/v2` |

## Pure compilation and funding

`compile_ordinal` validates every supplied basis/recovery/Template/Genesis/
Attachment/Series content join and the complete market-core
registry/capability/Realm projection. It checks the terminal `BURN` value and bearer lot, requires the
Market cap to be at least one native bearer lot, an exact multiple of that lot,
and no greater than the Realm ceiling, checks the final finite recovery
deadline, and derives an absolute schedule and full-width MarketInstanceId.
Series provenance and attachments remain next to the market result rather than
entering its economic preimage. `CompiledScheduleV1::validate` is the sole pure
owner of absolute schedule shape, ordering, generation, and padding checks.

A singleton Series has the sole canonical `stride = 0`; a multi-instance
Series requires a positive stride. This pure representation imposes no
otherwise-unexplained product-policy maximum on window span or occurrence
count: checked final-range arithmetic remains the exact admission bound, while
a live capability profile may impose and identity-bind stricter operational
limits.

`SeriesFundingTermsV1::validate_bindings` repeats the complete structural join
and additionally requires the exact Realm/Profile collateral mint, token
program, and canonical neutral incinerator. It does not authenticate or
duplicate the Realm collateral policy.

`SeriesFundingQuoteV1` is the sole exact owner of per-component amounts and
recovery work pricing. It binds the exact RecoveryPolicy ID, one positive
progress-cap/rate row per active policy attempt, zero padding, separately owned
recovery rent, and the checked aggregate recovery component. Changing a cap or
rate changes its typed digest even when the maximum aggregate work principal is
unchanged. `project_component_debits` accepts the expected recovery policy, the
exact quote, and an adapter-authenticated occurrence/Attachment/quote status. It projects
independent market-core, recovery-reserve, source-work, liquidity, and wrapper
debits with checked sums and segregated lamport/collateral balances. A mismatch
is never a status. Market core and mandatory recovery state must be created or
reused together. `PresentExactAndCapitalized` means the adapter already proved
the exact component identity, state, and required balances; the public Rust
value is not itself that external proof.

`SeriesFundingStateV1` is now the one mutable whole-Series owner for exact
activation principal, the ordinal cursor, lapse count, five segregated
remaining-principal/donation compartments, and absent-component allocation
consumption. It derives created count and phase rather than persisting them
twice. Activation joins the complete V5 Product/Genesis/Realm/FundingTerms
graph, requires exact `instance_count * quote_component` principal, and cannot
use donations to cure a shortfall. Creation derives the component projection
from a default-deny adapter authority, exact-existing components consume zero,
and lapse spends nothing. Applying those transitions atomically and moving real
lamports/tokens remain adapter obligations; the existing SBF route does not yet
implement them.

The pure Series join treats nonzero LiquidityFacilityPlan and WrapperRecipeSet
IDs as structural attachment references only. It does not authenticate those
plan bodies, prove their component quote amounts, or prove that the selected
runtime capability profile admits the exact liquidity-dealer and
structured-claim releases. Live attachment activation remains fail-closed until
an adapter-authenticated attachment-capability join supplies and checks those
authoritative bodies. This restriction does not alter `MarketInstanceId`, from
which operational attachments are deliberately excluded.

For the price-owning successor, the equivalent APIs are `compile_ordinal_v2`,
`SeriesFundingTermsV2::validate_bindings`, and
`project_component_debits_v2`. The V2 registry join additionally checks the
supplied exact quantized PriceMeasurePolicy body and requires its ID to equal the
Genesis and capability-owner IDs. A future RelationV2 adapter must set
`EconomicDomainV2.price_policy_digest` to the exact
`PriceMeasurePolicyV1Id` bytes, set price-measure `basis_digest` to the exact
`NativeClaimBasisV1Id` bytes, and bind the Genesis-owned coordinate range plus
the registry-resolved basis selectors into its authenticated Market/Economic
domain join. The candidate-price digest is derived from the canonical exact
candidate transcript—EconomicDomain digest, outcome count, price scale, and
active integer prices—not from the PriceGrid body. The independently
authenticated PriceGrid proves the common scale and venue tick membership.
This crate does not claim that adapter exists.

In particular, the Product-side d0 finite payout table is admitted without
loss, but witness activation still refuses until the adapter authenticates its
registry selector mapping and exact price transcript.

## Compatibility refusal and evidence

The legacy `DCTMPLV3` and `DCPAYTV3` magics return
`LegacyNumericFallback`. A current V3 Template/Payout body cannot be padded,
reinterpreted, or relabeled into these successor semantics.

Golden digests and every fixture input are frozen in
`vectors/product-series-v1.json`; the adversarial integration test independently
recomputes each digest and checks every manifest entry.

The successor adversarial suite freezes its exact lengths, magic/version
headers, policy versions and bounds, transitive identity behavior, and
cross-version refusals. V1 rejects V2 bytes and V2 rejects V1 bytes; equal-width families
refuse on fresh magic, while different-width families refuse before decoding.

```text
cargo test --manifest-path crates/clutch-product-series/Cargo.toml --offline --locked --release
cargo clippy --manifest-path crates/clutch-product-series/Cargo.toml --offline --locked --all-targets -- -D warnings
cargo doc --manifest-path crates/clutch-product-series/Cargo.toml --offline --locked --no-deps
```

Passing these tests is pure-core evidence only. There is no SBF route, account
codec, deployment, Token-2022 CPI, source authentication, or local-validator
claim in this crate.
