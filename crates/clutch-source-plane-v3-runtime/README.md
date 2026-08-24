# SourcePlane V3 runtime account contract

Status: **successor runtime/SBF implementation under unified all-or-none profile
composition; not a deployment claim**.

SourceSeries `77/v2` actions 1 through 12 now have explicit account contracts
and handlers over one release-selected route. Product privately capitalizes and
publishes the founding occurrence/request graph; one program-derived custody
pre-pays bounded liveness, every child/receipt rent source, terminalization,
and deterministic reopen work; Failure's sole ResolutionV5 outer reconstructs
the persisted successful handoff and performs terminal/result close atomically.
Product retirement is the sole consumer allowed to refund the custody's unused
principal and neutralize every other observed lamport. Central dispatch remains
the all-or-none capability owner and must not admit a partial lifecycle.

This crate replaces the old opaque/default-deny SourcePlane V3 seams with
fixed-memory contracts that a small Solana adapter can execute. It does not
import Solana SDK or oracle SDK types. Instead, the adapter supplies exact
runtime facts after reading accounts, deriving PDAs, invoking reviewed
programs, and reading sysvars. The crate then recomputes complete byte digests
and refuses partial or mismatched joins.

## Implemented route

`authenticate_source_route` joins all of the following at once:

- the exact `SourcePlaneProgramV3` content identity;
- the runtime adapter executable, its linked ProgramData account, complete
  account-byte digests, loader, and frozen deployment slot;
- the reviewed parser executable and ProgramData under the same checks;
- the Pyth receiver executable, linked ProgramData, complete account-byte
  digests, loader, and frozen deployment slot, authenticated again at ingest;
- the exact receiver Config address, owner, and complete current byte digest;
- immutable parser configuration and existing SourceSpec accounts, including
  address, owner, and complete byte digest;
- the mutable feed address and owner;
- the Product/failure program authorized to own exact initial/repair generation
  requests, plus the System Program used to recognize unallocated PDAs;
- immutable Clock, heterogeneous Source-work quote, liveness-policy, Source
  compartment, semantic-owner, and neutral-sink identities.

The deployment checks read the ProgramData link and deployment slot from exact
manifest offsets. They do not call a host release registry or accept a version
number as a substitute for reviewed bytes.

`authenticate_boundary` then joins the authenticated receiver release and
Config, one complete feed account, reviewed parser invocation/return-data
facts, the exact parser output, and an authenticated Clock snapshot. A
boundary is admitted only after its canonical bucket closes, before its
immutable lateness deadline, and while publication time and slot are inside
the frozen freshness bounds. Pinning the current Config selects an acceptable
governance state at read time; it does not claim action 4 posted the update or
prove which Config governed an older post.

SourceHead creation consumes an exact 168-byte immutable generation request
owned by the frozen Product/failure program. It binds SourcePlane, SourceSpec,
repair generation, first/required-last bucket, policy, and work schedule;
absence alone cannot select a start bucket or repair generation.

The parser invocation and PDA result structures are deliberately named
adapter attestations. Rust values are not proof: the SBF adapter must construct
them only from runtime CPI/return-data and canonical PDA derivation. Every
digest derivable from supplied bytes is recomputed here.

## Ingestion, evidence, and recurrence

- `BoundaryBatchV1` admits up to eight consecutive already-authenticated
  boundaries. `ingest_boundary_batch` appends the whole batch and can atomically
  freeze a `RawPageV3` and advance `SourceHeadV3`.
- immutable page accounts are hostile-decoded through the exact V3 account
  envelope, owner, PDA recipe, bump, and complete core body;
- up to four authenticated pages can advance `WindowWorkV3` in one bounded
  call;
- window sealing joins the exact maturity page, closure receipt, SourcePlane
  release, Clock maturity, and final `WindowSealV3`;
- evaluator authority pins executable and ProgramData bytes to one exact
  `SummaryProgramV3`; result authentication decodes the evaluator's exact
  returned `StatisticResultV3` and revalidates the key/summary/seal/window
  graph;
- `join_source_occurrence` consumes the Product/Series crate's exact 184-byte
  `DCSOCCV1` wire body. It does not persist another occurrence DTO. The private
  receipt binds SeriesPlanV5, ordinal, MarketInstanceV2, attachment, Window,
  StatisticKey, SourcePlane, SourceSpec, repair generation, and created versus
  exact-existing disposition.

SourceSeries `77/v2` action 10 exhaustively owns the three admitted V2 wire
variants. `FailureAbsence` authenticates a never-created result and missing
seal at the exact mature window; `FailureResult` requires an authenticated
persisted refused result; `SuccessfulEvaluation` requires the exact successful
result/evidence graph consumed by the private Product/Failure ResolutionV5
join. Wrong source identities or repair generations are binding refusals, not
failure evidence.

Result absence is not a boolean. It requires the predictable result PDA to be
an unallocated zero-balance System account and its authenticated durable
lineage to be in the never-created partition. A closed or previously created
result can never be relabeled as “absent.”

## Reopen and funding ownership

`ReopenLineageV1` is a durable exhaustive state partition: never created,
exactly one open generation, or closed with an exact terminal receipt. New work
accounts consume only the monotone next generation. Plain account absence does
not authorize recreation.

Source account rent accounting supports both ordinary and fully prefunded
addresses:

- the creator funds only `max(rent_minimum - balance_before, 0)`;
- only that exact shortfall is refundable principal;
- every pre-existing lamport and later surplus belongs to the neutral sink;
- a fully prefunded account has zero principal and no invented payer authority;
- close returns principal once and routes the entire remainder to the sink.

Mandatory work is held in the separate liveness Source compartment. A
`SourceWorkScheduleBindingV1` owns the exact schedule digest, aggregate
dot-product work capital, maximum calls, largest single-call ceiling, rent, and
four terminal-path bounds. Each `SourceWorkAuthorizationV1` binds a concrete
family receipt account, owner program, lifecycle, semantic owner, generation,
ordinal, and authenticated per-call ceiling for direct projection into the
liveness runtime. Product's pending SourceWork debit capitalizes the exact
Rent-derived complete-lifecycle quote before the Market root exists. Child,
policy, receipt, terminal, and post-terminal reopen creation use only the
schedule-selected custody PDA; keepers sign to accept work but never fund rent,
and refund destinations never sign.

## All-or-none integration boundary

The successor profile may expose actions 1 through 12 only together, after
every persisted action-10 variant has a complete terminal consumer, Product's
founder consumes the private capitalization/publication receipt, and Product's
counted retirement consumes the private Source-custody close receipt before
current Funding close. The successful Failure action-12 outer must consume the
persisted Source context and final ResolutionV5 postwrite. There is no legacy
SourceArchive/SourceGeneration fallback and no public receipt-ID projection.

No test, build, campaign, benchmark, or deployment claim is recorded for this
implementation slice.
