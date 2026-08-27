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

The consequence for Structured's six Token effect kinds is that they stop being
Rust and become `EffectProgramV4` operations resolved through the existing
`FixedRole` composition. Structured has no Claims child of its own -- its
single backing edge stops at the claim-shard layer -- so it adds no new role,
and therefore triggers no `hot_v3.rs` edit.

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

- No `hot_v3.rs` change is required or permitted by this route. The
  coordination stop the charter reserved against `DECOMP-r` is not needed.
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
