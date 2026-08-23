# Product compiler V1 model

Status: **MODEL / HOST ONLY**. This crate is not an SBF instruction family,
account codec, deployed program, source authenticator, or funding authority.

This crate supplies the missing deterministic product path between the product
thesis and the current account plane:

```text
authenticated SourceSpec projection + SummaryProgram
                         +
                    TemplateV1
                         +
    SeriesPlanV1 + prepaid SeriesFundingV1
                         |
             permissionless next ordinal
                         v
       InstanceId + Hatchery WindowKey
                         |
      current TermsAccount compatibility lowering
                         |
              current Market/Outcome IDs
```

It models both terminal and maximum-drawdown Templates. Terminal Templates can
be lowered into the current `TermsAccount` v3 statistic registry. Drawdown
Templates are canonical compiler artifacts but deliberately refuse that
lowering because the current accumulator and Terms-to-resolution registry do
not implement maximum drawdown. No numeric identifier is invented to make an
unsupported route appear live.

The model keeps one semantic owner for each persisted fact:

- SourceSpec owns source identity, adapter release, feed, and observation grid.
- SummaryProgram owns evaluator identity/version and the feature set it can
  actually derive.
- Template owns the relative observation span, statistic, partition, payout,
  ambiguity, edge, failure, and human-terms digest.
- immutable SeriesPlan owns recurrence, Realm/Profile, price/fee policy,
  market-nonce range, collateral cap, and references to funding policies.
- mutable SeriesFunding owns only remaining prepaid compartments and the next
  ordinal; future fees never enter its equations.
- the complete semantic descriptor owns Instance identity. SeriesId, ordinal,
  creator, and free nonces are absent, so independently scheduled identical
  Instances converge. Exact windows, legacy market IDs, Terms, outcome IDs,
  and liquidity-policy inputs are deterministic projections.
- Hatchery WindowKey owns the exact shared raw source window. Statistic and
  SummaryProgram are intentionally absent, allowing independently evaluated
  terminal and drawdown results to reuse one immutable window result.

`LiquidityBlueprintV1::bind_current_policy` bridges an instantiated market into
the existing `clutch-liquidity-policy-model`. The blueprint, not Series, owns
the risk region, quote compiler, inventory bounds, tranche cap, and withdrawal
policy. Series merely references its digest and prepays its exact per-Instance
collateral cap.

## Current-account boundary

The current `TermsAccount` is a compatibility output, not the proposed
Template account. It repeats facts that the normalized model references by
digest and is 1,656 bytes per distinct exact window. `lower_current_terms`
preserves that current byte contract and returns a validated account, but it
does not make Template, Series, or Instance live on SBF. The dispatcher and
account allocation routes must be designed and reviewed separately.

The current market identity accepts only a `u64` nonce. Compatibility lowering
derives it from the InstanceId rather than accepting caller choice, but this is
still a disclosed 64-bit truncation boundary. A future market identity must
bind the full InstanceId; a current adapter would have to detect/refuse the
unlikely but semantically possible truncation collision.

Current SourceArchive/Feed V1/V2 is structurally one-window state. The model
therefore requires a content-addressed Hatchery source-plane generation >= 3
with a source-only FeedHead, reusable raw pages, immutable windows, and derived
statistic results. A legacy capability artifact refuses; compiling one current
terminal Terms account is not evidence that recurring Series can run today.

## Verification

```sh
cargo test --manifest-path research/product-compiler-v1/Cargo.toml --locked
cargo clippy --manifest-path research/product-compiler-v1/Cargo.toml \
  --all-targets --locked -- -D warnings
```

`tests/adversarial.rs` freezes cross-object refusal vectors and
`vectors/product-compiler-v1.json` records the stable canonical identities of
the reference terminal and drawdown surface.
