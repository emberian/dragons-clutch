# Claim-check compaction — a market retires, its holders keep their money — 2026-08-30

## Result

A terminal market can now retire past a holder who never comes back, and that
holder still gets paid — the same number, to the atom, that redeeming on time
would have paid them.

Census row **R3** said the opposite: *one sleeping holder blocks retirement
forever*. The zero-supply gate at `market_closure_v1.rs:669-681` admits no dust
and no write-off, redemption is owner-signed by design, and no Clock is read
anywhere on that path — so a lost key held the market, its rent, and every
downstream recovery open with no bound. Ember ruled option **(c)** on it this
morning, as a values call rather than a mechanism one: *perpetual claim, not
perpetual account.* Claims do not expire.

What shipped is that ruling. After a release-fixed deadline anybody may open a
per-market escrow and then crank each sleeping position through **redemption's
own payout derivation**, paying the resolved collateral into an escrow only that
holder can open. The position and its admission record close, the supply they
held is retired, and the crank is paid out of rent that was leaving those
accounts anyway. The holder returns whenever they like — years later, to a
market whose every account is gone — signs with their own key, and is paid out
of seven accounts, none of which is the market's.

**One ELF changed: claims-sbf.** No Core, Trading, Registry or Custody change,
no Lean regeneration, no change to `Action` or `CoreState`.

## The assertion the feature exists for

Compaction does not compute a payout. It **calls** the holder's own terminal
settlement route, with two fields changed — `recipient_owner` and
`recipient_token_account` — and nothing else.

That is why the two paths cannot diverge, and the campaign proves it as a
number rather than as a claim. Both halves run against the same fixture: one
where the holder redeems normally, one where they sleep through the deadline
and a stranger cranks. Each figure is read off the chain.

```text
a_compacted_claim_check_is_worth_exactly_what_redemption_would_have_paid ... ok
```

A second author for the payoff function is how a compaction that pays a
different number than redemption would have gets built and passes its own
tests. The wire enforces the absence of one: the compaction request carries a
`TerminalSettlementRequestV3` **verbatim**, decoded by that header's own
decoder and by nothing else, so a corrupted byte inside is caught by the
terminal decoder rather than by a copy of it.

## Measured

| route | compute units | accounts |
|---|---|---|
| `OpenClaimCheckEscrow` | 33,768 | 12 |
| `CompactPositionToClaimCheck` | 499,809 | 42 |
| `RedeemClaimCheck` | 13,399 | **7** |
| `CloseClaimCheckEscrow` | — | 6 |

Redemption costs 37× less than the compaction that created it, and touches
seven accounts against forty-two. That gap is the design: the payout is
resolved once, at compaction, into a fixed-width record, because every input to
the payout function — the aggregate, the position, the linked basis record, the
composition graph record, the Hoard — is destroyed by the retirement the
claim-check exists to permit. A record storing raw per-outcome atoms would be
an IOU denominated in a function nobody can evaluate.

## The residue, stated exactly

Not *nothing* — an escrow and a vault survive for as long as anybody is owed,
and pretending otherwise would be the kind of claim this project does not make.
What is true is that the residue is proportional to unredeemed claims rather
than to the market, is width-independent, and is **self-liquidating**: the last
redemption is what allows the escrow's close, after which nothing remains.

An escrow still owing somebody cannot be closed at all (`0x5625`). That is the
ruling working as intended rather than a leak: the claim survives, so the
collateral has to be somewhere.

## Running it

The whole arc, against the real Claims, Core, Custody, Registry, Resolution and
Token-2022 ELFs:

```sh
cargo build-sbf --manifest-path programs/dclutch-claims-sbf/Cargo.toml \
  --sbf-out-dir "$SBF_OUT_DIR"

SBF_OUT_DIR=/absolute/sbf cargo test -p dclutch-claims-sbf \
  --test rational_representation_v2_program_test claim_check -- --test-threads=1
```

```text
test result: ok. 16 passed
```

`$SBF_OUT_DIR` needs `dclutch_claims_sbf.so`, `dclutch_core_sbf.so`,
`dclutch_custody_sbf.so`, `dclutch_registry_sbf.so`,
`dclutch_resolution_proof_sbf.so`, `dclutch_rational_v2_test_caller_sbf.so` and
the canonical `spl_token_2022.so`; `tools/gauntlet/claims-extended/run-claims-extended.sh`
builds all of them. That script also **refuses to run at all** if any artifact
carries an SBF stack-frame-overwrite diagnostic, which is why
`process_compaction` is split across two `#[inline(never)]` frames.

The one scenario that is the whole feature in a sentence:

```sh
SBF_OUT_DIR=/absolute/sbf cargo test -p dclutch-claims-sbf \
  --test rational_representation_v2_program_test \
  claim_check::a_market_retires_a_sleeping_holders_position_and_the_holder_is_still_paid \
  -- --test-threads=1
```

The contract layer, which needs no ELF and no validator:

```sh
cargo test -p dclutch-claims-svm --lib claim_check        # 61 passed
cargo test -p dclutch-claims-sbf --lib claim_check        #  6 passed
cargo test -p dclutch-operator   --lib claim_check        #  5 passed
```

And the refusal-code gate, which must see the seventeen new codes rather than
merely pass:

```sh
tools/gauntlet/census/target/release/dclutch-route-census inventory \
  --root . --out /tmp/inventory.json --revision "$(git rev-parse HEAD)" --check-unique
```

```text
census: 292 refusal codes declared across 130 packages, against 25 registered bands
```

Whole-file regression, including the 25 scenarios this lane did not write:

```text
test result: ok. 41 passed; 0 failed
```

## What the tests refuse to let a later edit do

- **Widen the redemption frame.** It is a declared role list, and every role must
  answer `survives_retirement()`. All seven answer true; an aggregate, a Core
  state, a basis or graph record, a Hoard or a Custody replay cursor cannot,
  because retirement closed them. A later edit reaching for one has to add a
  role and state the answer, and the honest answer fails the test.
- **Ask a second person to sign a redemption.** A test counts the signers in
  that frame and requires exactly one, and that it is the person being paid.
- **Redirect a sleeping holder's collateral.** The compaction recipient is
  derived from the market's own aggregate; naming the holder is refused outright,
  since that is their redemption with the signature deleted.
- **Block a crank with dust.** Every place a lamport can be sent — the position,
  the admission, the vacant claim-check address — is tested and absorbed. The
  route uses `allocate`/`assign` rather than `create_account`, which refuses an
  address that already holds lamports.
- **Shorten a live market's deadline.** It is a compiled constant in a
  digest-pinned ELF, and a live market's release set is write-once.
- **Mint a claim-check promising nothing.** Refused at the constructor, across
  the wire, and in conservation — because every holder of a losing outcome
  resolves to zero, and one such record would pin an escrow open forever.

## Debt, named

- §6.2's dust-tolerant close receipt is not written. Conservation is checked by
  the plan and the settlement's own receipt is the evidence, so this is an
  off-chain consumer's convenience rather than a safety gap. Still owed.
- **R3 is narrowed, not closed.** Native positions, yes; fractional ones, no —
  their claimants are the holders of a mint, plural and unknown to the position,
  and a one-owner claim-check cannot represent them. §10 of the design predicted
  this and states the shape of the V2 that would close it.
- The campaign does not drive `market_closure_v1` itself, because its market
  carries Claims capability positions that V1 compaction refuses by design. What
  it proves is the R3 claim precisely: after the crank the sleeping holder's
  position does not exist and its supply is gone from the aggregate.
- The campaign plants its admission record rather than driving a real `Admit`:
  this campaign's test caller has no Admit verb. The record is built with the
  production codec, and compaction reads exactly one field from it.

## Findings this lane returned to the wave

- **`create_account` refuses an address that already holds lamports**, which
  makes it a griefing vector for any stranger-computable PDA. `allocate` +
  `assign` is both the house idiom and the dust-tolerant one.
- **"market" names two different values** in this tree — the semantic
  `identity.market_id` and the Core state *account address* — and a position's
  aggregate seeds off the latter. Caught by a fixture, not by review.
- **`wallet_payout_custody_caller` hardcoded a value the request already
  carries** (`destination: terminal.recipient` against
  `input.recipient_token_account`). Identical on every wallet payout, which is
  exactly why nothing caught it, and wrong for any payout to any other
  destination. Now reads the request.

## Provenance

Design: `docs/design/CLAIM_CHECK_COMPACTION_V1.md`, including §15 (the six
amendments implementation earned) and §16 (what shipped, reconciled against what
was planned). Ruling: `docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:1183-1186`. Thirteen commits, `a2ad25ed`
through `dff38069`.
