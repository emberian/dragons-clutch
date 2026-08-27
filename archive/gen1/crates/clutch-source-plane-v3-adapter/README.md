# SourcePlane V3 adapter contract

Status: **PROPOSED / NOT LIVE**. This crate does not add dispatcher tags,
capability profiles, instructions, deployments, or a release manifest.

This is the fixed-memory contract between the existing
`clutch-source-plane-v3` semantic core and a future small Solana adapter. It is
`no_std`, allocation-free, safe Rust, exact-version-only, and contains no
Solana SDK, oracle SDK, borrowed `AccountInfo` parser, CPI, or signature logic.

## Semantic ownership

The V3 core remains the only owner of these facts:

- `SourceHeadV3`: source-only cursor, predecessor, repair generation, and last
  authenticated source facts;
- `OpenRawPageV3` / `RawPageV3`: immutable prefix and content-addressed page;
- `WindowSpecV3` / `WindowWorkV3` / `WindowSealV3`: statistic-neutral window
  semantics and exact reusable-page fold;
- `StatisticKeyV3` / `StatisticResultV3`: predictable request versus actual
  result content;
- `ProductTemplateV3`, `InstanceDescriptorV3`, `SeriesPlanV3`, and
  `SeriesFundingV3`: reusable economics, convergent absolute Instance,
  monotone finite schedule, and segregated prepayment.

The adapter prefix owns only account-family dispatch, PDA bump, and the shared
`TerminalIdentityV1` rent/donation generation. It never repeats a core field.
The fixed account image is:

```text
magic[8] | layout_version:u16=1 | family:u16 | bump:u8 |
flags:u8=0 | reserved[2]=0 | TerminalIdentityV1[56] | exact core body
```

Every family is associated with exactly one `FixedCodec` through
`AccountBodyV3`. Unknown later versions, trailing bytes, nonzero reserved
bytes, wrong family/body pairings, and hostile core bodies refuse.

## PDA proposal

`PdaRecipeV3` proposes disjoint seed namespaces but deliberately does not
derive an address: the live adapter must derive and compare under the exact
deployed program id. The critical recipes are:

| account | seed coordinates after prefix | reason |
| --- | --- | --- |
| existing V2 SourceSpec | `feed_id` | preserves exact live `[b"source-spec-v1", feed]` |
| SourceHead | `SourcePlaneContractId, SourceSpecId, repair_generation` | prevents release relabeling and segregates repair generations; authority is separate |
| OpenRawPage | `SourcePlaneContractId, SourceSpecId, repair_generation, page_index` | index comes from Head, never instruction data |
| RawPage | `SourcePlaneContractId, RawPageId` | immutable content address tied to reviewed release |
| WindowWork / WindowSeal | `WindowKey` | predictable shared window slot |
| StatisticResult | `StatisticKey` | predictable request address; stored body commits ResultDigest |
| Template / Series | their content id | immutable content address |
| SeriesFunding | `SeriesId` | one durable exact-next cursor |
| Instance | `InstanceId` | creator, Series, ordinal, and arbitrary nonce remain absent |

`SourcePlaneContractId` means the content identity of the reviewed core
release, not the deployed adapter Pubkey. The V3 core's already-frozen field is
named `source_plane_program_id`; adapters must preserve its bytes while keeping
that distinction explicit at the deployment boundary.

The SourceHead recipe includes repair generation, but that alone is not
genesis authority. `SourceGenesisAuthorizationV3` has private fields and this
crate version exposes no constructor. SourceHead initialization therefore
refuses operationally until the complete V2 route plus creation/repair
authority is promoted as a typed runtime capability. New source families need
their own explicit authority join as well.

## V2 binding and migration refusals

`project_v2_source_spec_fixture` checks the expected 404-byte V2 wire shape:
tag `0x73`, version `1`, stored feed, full 368-byte `DCSRCV2` model body,
stored bump, zero flags, and
`SHA256("dragons-clutch/feed/v2" || body)`. Its program, key, bump, and bytes
are caller-supplied, its codec dependency labels itself MODEL-ONLY, and the
frozen vector is self-generated. It is useful for host compatibility fixtures,
not runtime-produced differential evidence or account authentication. Live
promotion must use the current runtime verifier as sole authority.

`V2AuthenticatedSourceRoute` reserves the full immutable Terms and compiled
adapter/parser/receiver release join. `V2AuthenticatedRecord` reserves the
complete live V2 authentication transcript. Neither type has a public
constructor: the host decoder can create only an untrusted representation
candidate and cannot promote a digest into authentication evidence.

Migration is transcoding, never aliasing:

- V2 record bytes 0..8 store `bucket`; V3 stores `kind + reserved` there and
  derives bucket from page position.
- V2 archive commitments, Realm/Terms-bound window ids, Feed state, PDAs, and
  sealed pages are not V3 identities.
- V2 admits increasing sequence with decreasing receiver-write slot; V3
  deliberately refuses that stricter-incompatible record.
- V2 may admit a normalized endpoint above V3's `MAX_SOURCE_VALUE`; projection
  refuses rather than clipping.
- V2 seals at its window end after per-boundary time grace. V3 requires a real
  page chain and closure receipt reaching its explicit maturity bucket. A V2
  sealed page cannot manufacture that extra evidence.
- V2's global crossing-witness uniqueness is a provider assumption/falsifier;
  the onchain path authenticates the adjacent update it sees. The V3 adapter
  must not upgrade that into a false optimality/uniqueness claim.

## Pure transaction projections

`TransitionPlanV3` commits exact account-image before/after digests (including
family, bump, and `TerminalIdentityV1`), PDA recipe ids, terminal generations,
absent-target creations, terminal closes, header-derived principal owners, and
segregated creation/work/liquidity amounts. `AccountMutationV3` delegates
monotone donation observation to `TerminalAccountV1`; `AccountCloseV3`
delegates its exact payer-principal/neutral-surplus split to the same semantic
owner. Raw hostile account bytes cannot be hashed into an authoritative state
digest; only a typed canonical header/body pair can. The projection families
are:

- source-head initialization, page open, one V2-authenticated boundary append,
  and atomic page seal/head advance/open-work close;
- WindowWork creation, one immutable-page fold, and atomic seal/work close;
- terminal and drawdown result creation;
- Series activation, next-Instance creation, lapse, and advance over an exact
  independently existing convergent Instance.

Several families are deliberately unreachable from safe external code in this
version. V2 route/record/genesis capabilities, authenticated RawPage/Instance
reads, evaluator outputs, the WindowWork lineage authorization, and Series
activation/instantiation transfer graphs have no public constructors. This
prevents a seed-recipe digest, arbitrary nonzero hash, account absence, or four
untyped scalars from masquerading as runtime authentication.

Generation one is the only represented generation, but that alone is not
durable replay protection after close. In particular, WindowWork creation
requires the opaque lineage capability and therefore refuses operationally
until a durable tombstone/history owner exists.

Terminal and drawdown projection functions accept opaque evaluator outputs.
This crate version exposes no constructors for those outputs, so neither path
can publish a result until an evaluator authenticates the exact release-bound
page chain and record-stream root. Caller endpoints, summaries, and nonzero
digests are never sufficient evidence.

The account-creation accounting projection has four separate fields: rent
principal, downstream creation budget, prepaid mandatory work, and
funded-liquidity collateral. Collateral is never added to lamports. Future fees
and Hoard/claim principal have no input. Lapse moves only the Series ordinal;
unused finite allocations remain visible. `project_refund_series_funding`
explicitly returns `SeriesTerminalRefundUnavailable` until a typed refund,
vault-close, collateral-return, and terminal account-close graph exists.

`IntentPreimageV3` commits the exact deployed adapter program, complete
transition-plan id, permissionless transaction submitter, exact Series
bucket/ordinal where applicable, and an exclusive slot expiry. The submitter is
not a rent-principal owner or Series funder. Bucket and ordinal are derived from
the plan rather than accepted twice. It has no free nonce. Replay resistance
comes from the program binding and plan's exact before-state, while independent
descriptions of the same Instance converge. Hostile-decoded intents must be
rejoined with `validate_for_program`, including the actually executing program
id; checking only an action or digest is insufficient.

These plans are semantic/accounting projections, not executable Solana account
meta lists. Transfer-dependent Series projectors require opaque commitments to
complete typed graphs; because no safe constructor exists yet, they cannot be
used to move a different asset or destination while matching the scalar plan.

## Structural width measurement

This is a byte-structure comparison, not a rent quote or a complete live-market
account plan. The current V3 layout constants are 1,656 bytes for `Terms` and
726 bytes for `Market`, or 2,382 bytes when both are duplicated per market. A
headed V3 `InstanceDescriptor` is `72 + 248 = 320` bytes. Replacing those
duplicated immutable facts with shared Template artifacts plus the compact
Instance descriptor therefore removes **2,062 structural bytes per Instance**.
Shared artifact accounts, mutable trading/vault state, allocator overhead, and
the cluster rent schedule remain outside this measurement.

## Still required for live promotion

A future SBF adapter must, in one transaction where the projection says so:

1. decode exact instruction/account versions and authenticate every owner,
   writable/signer bit, PDA, stored bump, sysvar, and reviewed program release;
2. promote the existing V2 source authentication path into the reserved opaque
   capabilities rather than treating the model-derived host decoder as proof;
3. derive bucket and page index from authenticated state, enforce intent expiry,
   compare the complete projected before state, and supply the actual account
   balance used by `AccountMutationV3` / `AccountCloseV3`;
4. implement prefund-safe System allocation and exact rent-shortfall
   accounting around the already-kernel-derived terminal observation/split;
5. implement and construct checked typed activation, instantiation, and refund
   graphs that bind debit sources, lamport destinations, Realm-selected
   collateral mint, vault owners, and terminal disposition atomically;
6. compute terminal/drawdown evidence from the authenticated page chain; and
7. add global dispatcher tags and capability profiles only after bank/SVM
   differential tests and a checked release manifest exist.
