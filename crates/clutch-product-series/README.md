# Product and Series pure core

`clutch-product-series` is the registry-independent, allocation-free semantic
core for recurring Dragon's Clutch products. It freezes typed content
identities, exact product/Series joins, absolute window and evidence-only repair
arithmetic, and component-by-component funding projections. It has no account
tags, instruction intents, Solana SDK, Token-2022, oracle SDK, CPI, account
memory, allocator, floats, or caller-selected market nonce.

The crucial identity split is:

```text
MarketInstanceId = H(TemplateId, MarketGenesisProfileId, start, cap)

SeriesFundingQuoteId   = H(exact per-component lamport/collateral amounts)
SeriesAttachmentPlanId = H(FundingQuoteId, LiquidityPlanId, WrapperSetId)
SeriesPlanId           = H(TemplateId, GenesisProfileId, AttachmentPlanId,
                           finite recurrence, cap)
SeriesFundingTermsId   = H(SeriesPlanId, refund/sink/mint/token identities)
```

Work price, liquidity, wrappers, funding sponsor, and refund destinations do
not fork an economically identical market. Changing the Template, immutable
Realm/profile venue semantics, absolute start, or collateral cap does.

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

### Remaining exact bodies

| Type | Bytes and offsets | Domain |
|---|---|---|
| `MarketInstancePreimageV1` | 88: magic `0..8`, Template ID `8..40`, GenesisProfile ID `40..72`, start `72..80`, cap `80..88` | `dragons-clutch/market-instance/v1` |
| `SeriesFundingQuoteV1` | 264: header/count `0..16`, RecoveryPolicy ID `16..48`, five ordered `(lamports u64, collateral atoms u64)` components `48..128`, recovery rent principal `128..136`, eight `(progress cap u64, lamports/unit u64)` rows `136..264` | `dragons-clutch/series-funding-quote/v1` |
| `SeriesAttachmentPlanV1` | 112: 16-byte header, FundingQuote ID `16..48`, LiquidityFacilityPlan ID `48..80`, WrapperRecipeSet ID `80..112` | `dragons-clutch/series-attachment-plan/v1` |
| `SeriesPlanV4` | 152: 16-byte header, Template/Genesis/Attachment IDs `16..112`, first start `112..120`, stride `120..128`, count `128..132`, reserved `132..136`, lead `136..144`, cap `144..152` | `dragons-clutch/series-plan/v4` |
| `SeriesFundingTermsV1` | 208: 16-byte header, Series ID `16..48`, lamport refund `48..80`, collateral refund token account `80..112`, neutral sink `112..144`, collateral mint `144..176`, token program `176..208` | `dragons-clutch/series-funding-terms/v1` |

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

This projection is intentionally for one occurrence only. It is not Series
activation, prepayment, fulfillment, or refund authorization. A separate
mutable whole-Series funding owner must bind total activation funding, cursor,
component receipts, lapse, and refunds before any live adapter may claim that a
finite Series is prepaid or fulfilled. Applying a one-occurrence projection
atomically, proving absence or exact capitalized existence, preserving
payer/donation ownership, and moving real lamports/tokens remain adapter
obligations.

The pure Series join treats nonzero LiquidityFacilityPlan and WrapperRecipeSet
IDs as structural attachment references only. It does not authenticate those
plan bodies, prove their component quote amounts, or prove that the selected
runtime capability profile admits the exact liquidity-dealer and
structured-claim releases. Live attachment activation remains fail-closed until
an adapter-authenticated attachment-capability join supplies and checks those
authoritative bodies. This restriction does not alter `MarketInstanceId`, from
which operational attachments are deliberately excluded.

## Compatibility refusal and evidence

The legacy `DCTMPLV3` and `DCPAYTV3` magics return
`LegacyNumericFallback`. A current V3 Template/Payout body cannot be padded,
reinterpreted, or relabeled into these successor semantics.

Golden digests and every fixture input are frozen in
`vectors/product-series-v1.json`; the adversarial integration test independently
recomputes each digest and checks every manifest entry.

```text
cargo test --manifest-path crates/clutch-product-series/Cargo.toml --offline --locked --release
cargo clippy --manifest-path crates/clutch-product-series/Cargo.toml --offline --locked --all-targets -- -D warnings
cargo doc --manifest-path crates/clutch-product-series/Cargo.toml --offline --locked --no-deps
```

Passing these tests is pure-core evidence only. There is no SBF route, account
codec, deployment, Token-2022 CPI, source authentication, or local-validator
claim in this crate.
