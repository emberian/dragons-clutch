# The admitted caller authority is seeded by a slot-bearing digest — 2026-09-03

**Design note, not evidence.** It names one protocol gap, its instruction, its
refusal code, its authority, and the shape of the fix. Nothing here is a
measurement of mainnet and nothing here authorizes a program change.

## The claim

Seven of the fifteen General actions — `OpenBatch`, `CloseBatch`, `PlaceOrder`,
`CancelOrder`, `ReleaseOrder`, `SubmitCandidate`, `CloseCandidate` — cannot be
delivered as a transaction on any real chain. Not because a producer is
missing, and not because the founded Market is wrong: because the top-level
account list must NAME addresses that are a function of the slot the
transaction executes in, and a signed transaction's account list is fixed when
it is signed.

`OpenBatch` is the first act of the General batch lifecycle, so the wall is at
the family's entrance, not somewhere inside it.

## The instruction, the conjunct, the code

`programs/dclutch-trading-sbf/src/admitted_composition_v3.rs`, in the
per-invocation loop that CPIs the admitted accelerator:

```rust
let request_digest = content(
    buffers.instruction.data.get(..request.signed_prefix_len()?)?,
)?;
let authority_seeds = CallerAuthoritySeedsV1::new(
    context.release_set,
    context.market.to_bytes(),
    ExecutionRoleV1::Trading,
    context.root.to_bytes(),
    request_digest.to_bytes(),
)?;
let (expected_authority, bump) =
    Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
if caller_authority.key != &expected_authority
    || caller_authority.is_signer
    || caller_authority.is_writable
    || caller_authority.executable
{
    return Err(TradingSbfError::Release.into());
}
```

- **Refusal band**: `TradingSbfError::Release`, `0x4001`
  (`programs/dclutch-trading-sbf/src/lib.rs`, band 4).
- **Conjunct**: `caller_authority.key != &expected_authority`.
- **Where the account comes from**: `carve_strategy_frame_span_v3`'s
  `AdmittedAot` arm slices `accounts[HOT_ADMITTED_CALLER_AUTHORITIES_START_V3 +
  displacement ..]` out of the **top-level** instruction's account list
  (`hot_v3.rs`; the constant is 47, i.e. 39 fixed + 8 strategy evidence). One
  account per accelerator invocation. They are pure `invoke_signed` PDA
  signers — no account has to exist at those addresses — so nothing needs
  *producing*. What is needed is that the caller can *state* them.

## Why the address moves every slot

`signed_prefix_len` covers the accelerator request header **and the inline
bank**; the prelude witness rides outside it, deliberately, "because the
caller-authority account whose address this seeds is derived off-chain by a
producer that cannot compose it" (the comment beside the digest).

The inline bank is `encode_register_bank(view.scalars, view.identities)` over
`replan_output_scalars` / `replan_output_identities` — the register bank
Trading builds in-instruction. `observe_trusted_environment_v3` reads
`profile.trusted_environment()`; for a profile declaring
`TrustedEnvironmentV2::CurrentSlot { destination }` it calls `Clock::get()`,
and `seed_trusted_environment_v3` writes that slot into
`scalars[destination]`. For General, `destination` is
`scalar::CURRENT_SLOT` = 90, inside a 151-scalar bank
(`crates/dclutch-general-adapter-contract/src/account_rules_v3.rs`).

So `sha256(header ‖ bank)` differs in every slot, and so does every
`find_program_address` over it.

Which actions declare it is decided in one `matches!` in
`general_account_profile_bytes_v3`'s encoder call: the seven listed above take
`CurrentSlot`, the settlement seven (`Consider`, `Freeze`,
`InitializeSettlement`, `Collect`, `Materialize`, `Distribute`, `Close`) take
`TrustedEnvironmentV2::None`. Their banks are slot-independent and their caller
authorities are, in principle, stateable — but every one of them is downstream
of a batch that `OpenBatch` has to open.

## The tree already states this law, and applied it once

`account_rules_v3.rs`, the test
`the_window_gated_actions_declare_the_current_slot_in_their_bank`:

> The window-gated actions put the CURRENT SLOT in their register bank. This is
> small and it is the reason the input scratch-page transport could never have
> a producer. … so the bank's bytes, and every digest over them, are different
> in every slot. **Anything outside the executing instruction that has to STATE
> that bank is therefore valid for exactly one slot, which no caller can
> deliver into.**

That is exactly this. It was used to delete the input scratch-page transport
(`1fee82fa`, `a517d27c`, `docs/design/GENERAL_INPUT_TRANSPORT_2026_09_02.md`);
the caller-authority address is the same kind of object — a value outside the
executing instruction that has to state that bank — and it survived the cut,
because the cut was reasoned about page *accounts* and this is a page-less PDA
*address*.

The reason it went unnoticed for so long is the same reason `M-40` records
`build_general_hot_instruction_v3` as having zero callers: General's admitted
route has only ever run inside `ProgramTest`, where a fixture builder computes
the bank at the same instant the runtime does. Direct, the one family that has
executed on a real chain, is `StrategyDispositionV2::Interpreted` and derives
no caller authority at all. General is the first family that would need one.

## Why the obvious workarounds are not routes

- **Predict the slot.** A transaction is valid for ~150 slots; the caller would
  have to name the exact one. That is a fee-burning lottery, not a route, and
  it would have to be won again for every action in the lifecycle.
- **Compute it later.** The account list is part of the signed message. A v0
  lookup table does not help: the table's contents are fixed too.
- **Let Trading create the account.** It never has to — the authority is an
  `invoke_signed` seed set, not an account. Creating something would not make
  the address stateable.
- **Drop the trusted current slot.** The window gate is the semantics: the
  batch actions are the ones whose admissibility depends on the collection and
  selection windows, and `require_trusted_environment_v3` exists so a caller
  may not state what time it is. Removing it moves the defect into the
  semantics.

## The fix, and whose decision it is

The caller authority's purpose is to bind the CPI's signer to *this exact
execution intent*, so that one authority cannot sign for another request. That
property does not require the trusted-environment bank; it requires a digest
over what the caller committed to.

Trading already carries such a digest, and already puts it in the bank:
`HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3` is
`sha256(family request)` — the caller-stated `DCLTHOT3` payload, fixed at
signing time. The proposed seed is

```
role_request_digest := sha256(parent_request_digest ‖ chunk_index_le)
```

which keeps per-invocation separation (the chunk index is what varies between
the four CPIs of one execution), keeps the binding to the exact request, and
is computable by any caller that can build the transaction at all.

What this costs and what it gives up, stated plainly:

- The authority would no longer be bound to the exact *accelerator request
  bytes*, only to the family request that determines them. Everything else in
  those bytes is derived by Trading from authenticated artifacts and chain
  state inside the same instruction, so the accelerator's own frame checks and
  `require_admitted_bank_matches_frame_v3` (`0x4018`) still cover the bank; the
  authority stops being a second, redundant statement of what Trading just
  computed and becomes a statement of what the caller asked for.
- It changes a PDA seed, which moves every admitted caller-authority address.
  Nothing on any chain depends on those addresses today — no admitted-AOT
  execution has ever run outside `ProgramTest` — so the change is free now and
  will not be later.

**Authority**: this is a Trading program change plus a
`CallerAuthoritySeedsV1` usage change, so it belongs to whoever owns
`admitted_composition_v3.rs` and `crates/dclutch-release-set-contract`. This
lane did not make it. The GENERAL-SESSION lane's remit was the host side, and a
host driver aimed at this route would have been a driver aimed at a refusal.

## The second, smaller wall that sits in front of it

Independent of the above, cohort-14's founded General market
`8ExdC1RwbyuJweEqT1F6Gk9rgN87uuVaLwtaY2wmr5x` publishes an `OpenBatch`
AccountProfile whose RentCredit coordinate is `Exact(48)`. 48 is the width in
`account_rules_v3.rs`'s own unit-test fixture, copied into the devnet policy
file; the only RentCredit the protocol produces is
`LIFECYCLE_RENT_CREDIT_BYTES_V2` = 128, and the market's own lifecycle
RentCredit `7FtTqxsy5888L8V9KSqvZj867UwD6PtRKeDtxeh92p21` is 128 bytes on
chain.

That is a **founding input**, not a protocol gap: the fix is to re-found with
widths observed from the cohort. It is recorded here only so the two are not
confused — repairing it would expose the caller-authority wall, not remove it.
`devnet-general-session` reports both, in order, for exactly that reason.

## How to check this note

```
dclutch-local-successor-bootstrap devnet-general-session \
  --rpc-url URL --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --plan $JOB/plan.json --market 8ExdC1RwbyuJweEqT1F6Gk9rgN87uuVaLwtaY2wmr5x \
  --result-domain-record HygbpeDNybaVmi77wG2hc89ZkqzSzE8z74ykTyXX27w4 \
  --portfolio-record RD3dehGUufAJkW3g6o4kdLkqXRMCYg2Ft4vQQoutLHG \
  --linked-basis-record JBpYRqC929AxdgeK8uaZBSe4XXpqXwt5ENUt1fGrMw21 \
  --payer $CAMPAIGN_PAYER --output $JOB/general/openbatch-frame.json
```

Read-only: it reads no keypair, signs nothing, submits nothing. It writes the
frame report and exits non-zero naming every unsatisfiable conjunct it found.
