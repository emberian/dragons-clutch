# Structured portfolio receipt V1

## Status and product boundary

This crate is the allocation-free semantic contract for an **optional,
transferable Token-2022 receipt** backed by an exact Product portfolio. It is
not an SBF program, has not executed a Token-2022 CPI, and is not yet an
offered dClutch capability. The receipt is useful only because ordinary
Token-2022 transfer and venue custody can move it independently of Structured
instructions.

The repository's current external SBF adapter implements Activate, Wrap,
Unwrap, and Retire, but deliberately refuses RedeemTerminal until its Realm
collateral-vault payout slice is integrated. That refusal is correct: burning
the receipt and persisting Market claim debits without moving winner
collateral would counterfeit settlement. Consequently, pure-contract terminal
completeness is not yet physical lifecycle completeness.

The capability remains deferred from release manifests, operator creation
flows, and UI claims until the required SBF adapter has:

1. parsed real Token-2022 Mint and Account bytes, extensions, ownership, and
   initialization state into the shared hostile Bearer observations;
2. authenticated every content ID, PDA, program owner, signer, writable
   account, generation, capability entry, and canonical Realm Position;
3. executed each native Position/Market mutation and Token-2022 mint or
   permissioned burn atomically, then reloaded accounts and enforced every
   returned postcondition before success;
4. demonstrated rollback under failed CPIs and adversarial substitutions on a
   local validator; and
5. demonstrated ordinary transfer followed by recipient unwrap and terminal
   redemption, plus one real generic Token-2022 venue/custody workflow.

If those gates cannot be met without a parallel supply truth or privileged
transfer path, Structured V1 must be deleted rather than advertised.

## Instrument

One zero-decimal receipt atom represents one exact integral materialization of
a canonical `PortfolioTemplateV1<N>`. For Product denominator `D` and
normalized, nonnegative numerators `c[i]`, Product defines rational quantity
`c[i] / D` at one Product scale. Structured derives the least realization lot

```text
L = lcm_i(D / gcd(D, c[i]))
```

and admits the template only when `L == D`. Materializing `D` Product-scale
units therefore produces the exact native categorical claim vector `c[i]`.
This denominator is exposed as `minimum_realization_lot`; it is not a token
decimal or an independently configurable conversion rate.

This is the honest successor to “Fractional”: rational recipes first
materialize at their exact denominator into integer backing, so the runtime
never creates fractional collateral liabilities or remainder credits.

### Named rounding boundary

Structured introduces **no rounding boundary**. Its one accepted conversion
boundary is `ExactDenominatorMaterializationV1`: multiply the Product rational
recipe by its canonical denominator and require every native claim quantity to
divide exactly. A remainder is a refusal, never a floor, ceiling, nearest-value
choice, or credit record. If the portfolio came from a graded Product, the one
system rounding/projection boundary remains Product Compiler's authenticated
`GradedRoundingBoundaryV1`, upstream of the finalized result domain and
PortfolioTemplate. Structured consumes that committed Product result and may
not round it again.

Let `T` be the observed Token-2022 Mint supply and `C[i]` the descriptor-owned
custody Position balance. The complete backing invariant is:

```text
C[i] = T * c[i]  for every outcome i
```

All products and sums use checked `u64` arithmetic. The Market's existing
aggregate categorical supply remains the sole native liability total and must
bound every visible Position. The receipt Mint supply is the sole structured
unit total; Token-2022 Account amounts are the sole holder balances. There is
no `StructuredSupplyV1`, `StructuredHoldingV1`, per-holder Structured record,
or other shadow ledger.

An ordinary external Token-2022 transfer changes neither `T` nor `C`. It
therefore preserves backing, and the recipient can authorize unwrap or
terminal redemption from their normal Token Account. Structured deliberately
has no bespoke transfer action.

## Semantic ownership

| Fact | Sole owner | Structured treatment |
| --- | --- | --- |
| Claim basis and result domain | Product Instance | Joined to Market and template; not persisted again |
| Coefficients and denominator | Product `PortfolioTemplateV1<N>` | Hostile-decoded and authenticated; only the content ID is persisted |
| Native categorical aggregate supply and payout | Market | Existing `redeem_outcome` semantics are called unchanged |
| Native per-owner and custody balances | Realm `PositionV1<N>` | Moved exactly; no alternate Position format |
| Structured total supply | Token-2022 receipt Mint | Consumed as an observation; never mirrored |
| Holder balance and transferability | Token-2022 Account/program | Consumed as an observation; ordinary transfer stays outside Structured |
| Capability selection and prepaid creation/rent | Capability manifest | Exact coordinate and typed quote are authenticated |
| Recovered rent destination | immutable `StructuredConfigV1` | Repeated in the descriptor only to make every close self-authenticating |

The SBF adapter must authenticate a template supplied as calldata or from a
finalized Product record by computing:

```text
SHA-256(
    dclutch_product_contract::portfolio::PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1
    || 0x00
    || canonical_template_bytes
)
```

The canonical template is exactly `104 + 8*N - 16` bytes: 104 bytes for
`N=2`, 216 bytes for `N=16`, and widths between those endpoints in eight-byte
steps. Its computed ID must equal the descriptor's `portfolio_template_id`
before any coefficient is used. Product owns the namespace constant; this
crate defines no alias or competing template domain.

## Exact records

All integers are little-endian. Every decoder rejects prefixes, trailing
bytes, wrong magic/schema/profile, and nonzero reserved bytes. Identity fields
must be nonzero, and physical Market/Mint/controller/custody identities must be
distinct.

### `StructuredConfigV1` — 112 bytes

| Bytes | Meaning |
| --- | --- |
| `0..8` | magic `DCLTSTC1` |
| `8..10` | schema `1` |
| `10` | immutable receipt decimals `0` |
| `11` | exact-N profile `1` (`N=2..16`) |
| `12..16` | canonical zero |
| `16..48` | pinned Token-2022 program ID |
| `48..80` | shared Bearer receipt-profile semantic release |
| `80..112` | permanent RentCredit beneficiary |

### `StructuredDescriptorV1` — 352 bytes

| Bytes | Meaning |
| --- | --- |
| `0..8` | magic `DCLTSTD1` |
| `8..10` | schema `1` |
| `10` | outcome count `N` |
| `11` | exact-N profile `1` |
| `12..14` | selected capability manifest entry index |
| `14` | immutable receipt decimals `0` |
| `15` | canonical zero |
| `16..48` | canonical Market key |
| `48..56` | immutable Market generation |
| `56..64` | canonical zero |
| `64..96` | Product PortfolioTemplate content ID |
| `96..128` | capability config content ID |
| `128..160` | Structured semantic release ID |
| `160..192` | shared Bearer receipt-profile release ID |
| `192..224` | descriptor-derived Token-2022 receipt Mint |
| `224..256` | descriptor-derived mint/burn/close controller |
| `256..288` | canonical Realm custody Position key |
| `288..320` | descriptor-derived custody Position owner |
| `320..352` | permanent RentCredit beneficiary |

Coefficient bytes, denominator, claim basis, result domain, Mint supply, and
holder balances are absent because each already has one authoritative owner.

### Instruction — 32 bytes

The exact instruction codec uses magic `DCLTSIX1`, schema `1`, action and
outcome-count bytes, four zero reserved bytes, generation, and one `u64` value.
The value is a nonzero unit quantity for Wrap, Unwrap, and RedeemTerminal, or
an expected-prior-child-count replay guard for Activate and Retire. Actions are
exactly Activate, Wrap, Unwrap, RedeemTerminal, and Retire.

## Identities and derivations

The following constants are SHA-256 digests of the exact ASCII preimages in
the implementation and are independently rechecked in tests:

| Coordinate | Exact preimage | Digest |
| --- | --- | --- |
| capability kind | `dclutch:capability-kind:structured-portfolio:v1` | `5f02472bbd21e546a2f86b4cc45a538bb4f7e18e6d973506bb177c49544a7116` |
| semantic release | `dclutch:structured-contract:semantic-release:v1` | `33a6eb591994287dd5a6a09ebd37f34d6b2ff42c294167e65268b13b2169aa86` |
| capacity profile | `dclutch:structured-contract:capacity:n2-n16:v1` | `1bdf67ea33cb6ac480c36f0ee1ccd7960c4df9b160d7e4db9c11adc552486200` |
| child schema | `dclutch:structured-contract:child-schema:v1` | `87ffcdbcf95fab03860217f407e34fa2889275fe6a79084f6a5fac16fef9c15b` |
| child derivation | `dclutch:structured-contract:child-derivation:v1` | `accd51747c3f2ba4b320d084ac20d026aaff5a497b58c3d51d16fdf4238d6a6b` |

The future adapter derives the descriptor from the ordered seeds:

```text
dclutch/structured/v1
Market key
generation LE
PortfolioTemplate content ID
config content ID
Structured semantic release ID
```

The receipt Mint, common mint/PermissionedBurn/close controller, and custody
owner are each derived from their respective domain plus the descriptor key:
`dclutch/structured-mint/v1`, `dclutch/structured-authority/v1`, and
`dclutch/structured-custody/v1`. The adapter must also derive the custody
Position with Realm's canonical Market/generation/owner Position rule and
match the descriptor field.

The receipt Mint uses zero decimals and the shared Bearer closed profile:
Token-2022 owner, initialized Mint, exact 238-byte width, common mint and close
authority, no freeze authority, and exactly the required MintCloseAuthority
and PermissionedBurn extensions. Permissioned burn prevents holders from
destroying receipt supply without simultaneously removing backing. It does
not restrict an ordinary Token-2022 transfer.

## Transitions

Every transition stages copies and commits only after all joins, arithmetic,
native mutations, receipt deltas, and postconditions succeed.

| Action | Allowed phase | Exact semantic effect |
| --- | --- | --- |
| Activate | Founding or Open | Register one guarded Market child; plan creation of the immutable descriptor, zero-supply receipt Mint, and empty descriptor-owned Position |
| Wrap `q` | Open or Resolved | Debit owner and credit custody by `q*c[i]`; require Mint supply and destination Account to increase by exactly `q` atomically |
| Unwrap `q` | Open, Resolved, or Retiring | Permissioned-burn exactly `q`; debit custody and credit the owner's canonical Position by `q*c[i]` |
| RedeemTerminal `q` | Resolved or Retiring | Permissioned-burn `q`; debit every nonzero `q*c[i]`; call Market redemption for every backed outcome; aggregate only canonical winner payout |
| Retire | Retiring | Require observed Mint supply zero and custody empty; decrement guarded child count; close descriptor, Mint, and custody only to RentCredit |

Terminal redemption consumes losing as well as winning native claim supply. A
losing redemption must return zero and a winning redemption must return its
exact claim amount; any different payout refuses the whole candidate. Losing
supply therefore cannot remain stranded in custody after receipt supply is
burned.

The manifest entry must select the exact kind/release/config/capacity/schema/
derivation coordinate and declare zero dependencies. Only `rent` and
`creation` may be `NativeLamports` (or not applicable). Work, provider,
bounty, liquidity, and service amounts must be zero, and Realm collateral is
forbidden. Activation may debit only that already-prepaid typed native
funding. Hoard principal and future fee revenue are never rent, creation, or
liveness capital.

## Bounds and lifting path

V1 supports exact categorical widths `2..=16` because that is the current
fixed-width Product template profile, not because Structured portfolios are
mathematically limited to sixteen outcomes. Lifting the bound requires a new
wider fixed layout or authenticated paged Product template, a new capacity
coordinate and descriptor profile, and corresponding measured SBF frame/CU/
account limits. It must not introduce truncation, dynamic allocation in this
kernel, or a second coefficient owner.

The one-unit zero-decimal scale is deliberate: a receipt atom is already the
least exact integer materialization. A future profile may represent a
different immutable exact scale only with a distinct release and proof that
its backing remains integral; mutable decimals or remainder accounts are not
an extension mechanism.

## Adapter completion plan

The remaining adapter vertical is small SBF boundary work, not more state in
this crate. Existing actions and the missing RedeemTerminal route must:

- parse exact descriptor/config/Product/Market/Position records and shared
  Bearer Mint/Account observations from hostile account bytes;
- recompute descriptor/config/template content IDs with their owning domain
  constants and the required zero separator;
- derive every PDA using the returned seed projections, check exact program
  owners, account lengths, executable program identity, distinctness, signer
  authority, and writable privileges;
- authenticate the capability manifest index and its typed prepaid funding;
- invoke this pure transition on staged account values;
- execute Token-2022 MintTo or PermissionedBurn using the descriptor controller
  in the same transaction as every native Position/Market change;
- reload Mint and holder Token Account and enforce the exact before/after
  supply and amount plan, then re-run backing audit; and
- refuse success unless all closes pay the immutable RentCredit and the Market
  child-count postcondition matches.

Transfer itself is intentionally normal Token-2022 behavior. Venue
integration should consume the receipt Mint as a standard zero-decimal asset;
Direct and General need not acquire a new Structured-specific settlement path.

## Evidence in this crate

The pure tests cover:

- every release/preimage digest and the Product-owned template namespace;
- exact release admission, including kind/config/capacity/schema/derivation,
  dependency, and typed-funding substitutions;
- exact config, descriptor, and instruction round trips plus every truncated
  prefix, trailing bytes, bad header/profile, dirty reserved byte, and zero
  identity;
- same-width template, claim-basis, result-domain, config, Mint, authority,
  Position, generation, and account substitutions;
- exact denominator-derived minimum lot and normalized coefficient backing;
- wrap/unwrap conservation and checked overflow/underflow with refusal
  atomicity;
- an ordinary external Token-2022 transfer observation followed by recipient
  unwrap and terminal redemption;
- winner and loser claim consumption, winner-only payout, zero-supply/empty-
  custody retirement, and refusal while either remains live; and
- complete Activate/Wrap/resolve/RedeemTerminal/Retire execution at every
  admitted exact width `N=2..=16`, plus refusal at `N=1` and `N=17`;
- stale activation, wrap, terminal-redemption, and retirement replay refusal
  with every pure Market/Position candidate unchanged; and
- fixed-copy layouts without allocation or unsafe code.

These are pure observation tests. They are not evidence that the Token-2022
program executed, that SBF account validation is correct, that transaction
rollback works, or that a venue accepts the asset. Those claims require the
adapter and local-validator gates above.

## Compost provenance

Historical Dragon's Clutch material was studied only to retain invariants and
identify counterexamples. The common inspected source snapshot was commit
`d95088bcb116d2321f3bfacf95f32031c814f4e5`:

- `research/structured-claim-wrapper/README.md`
- `research/structured-claim-wrapper/ADAPTER_PLAN.md`
- `crates/clutch-structured-claim/README.md`
- `crates/clutch-fractional-redemption-runtime/README.md`
- `LICENSE`

That snapshot is AGPL-3.0. dClutch is AGPL-3.0-or-later. No Dragon's Clutch
source code, byte layout, state machine, account DTO, or generated fixture was
copied; this crate is a fresh implementation against dClutch Product, Market,
Realm, Capability, Bearer, and Token identity contracts.

Retained invariants were: exact integer backing; immutable recipe binding;
custody conservation; total terminal consumption of both winning and losing
claims; replay-guarded lifecycle closure; and a permanent rent beneficiary.
Their new semantic owners are Product PortfolioTemplate, Market, Realm
Position, Token-2022 Mint/Account, Capability manifest, and Structured
descriptor respectively. The new public API is the descriptor/config and
instruction codecs, joined Product/context constructors, PDA seed projections,
backing audit, and the five transition plans documented above. The new layouts
are the exact 112/352/32-byte layouts in this document, and the adversarial
tests named above cover each retained invariant.

Explicitly rejected assumptions from the compost are:

- B-spline or other native-shaped basis semantics inside Structured;
- residual/cash compression or wrapper-specific payout authority;
- fractional liabilities, remainder credits, or rounding-ledger state;
- wrapper nesting and recursive authority graphs;
- a nontransferable holder ledger masquerading as a market instrument;
- direct uncoordinated burns that can leave donated or surplus backing;
- bespoke transfer or privileged venue settlement;
- mock account sources or client/index authority; and
- parallel persisted coefficient, supply, holder, resolution, or collateral
  truth.
