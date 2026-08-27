# Successor immutable-infrastructure bootstrap

This standalone localhost utility prepares the exact immutable substrate for
the current multi-program successor and then drives the first market lifecycle
through a real local validator. It does not seed mutable protocol state or call
a legacy direct Resolution ABI.

The prepared plan binds seven pairwise-distinct, real SBF artifacts:

- Registry
- Core
- Claims
- Trading
- Resolution
- Custody
- RentCredit

Registry, Claims, Trading, Resolution, Custody, and Rent are represented by
immutable Loader-v3 Program accounts and exact fixed-offset ProgramData
headers followed by the exact ELFs. Core begins with the same exact ELF and a
single ephemeral upgrade authority, then must reach that immutable header by
Loader revocation before release recognition. The plan also creates distinct
`ArtifactReleaseV1` bodies, the five-role
`ExecutionReleaseSetV1`, the captured local-Pyth release body, and the expected
144-byte `ProtocolInfrastructureProfileV1` body selecting Registry and Rent.
The profile itself is not genesis-injected: its sole PDA is derived under Core
and must be created by the canonical initialization transaction.

## Evidence boundary

Only Loader accounts and the infrastructure release records required to start
the successor are prepared as genesis fixtures. Core's genesis ProgramData is
explicitly pre-init and not an accepted immutable release observation. The
supervisor executes the remaining infrastructure and market boundary as real
localhost transactions:

1. Core initialization of the sole Registry/Rent infrastructure profile.
2. Loader-v3 revocation of Core's ephemeral authority to `None`, followed by
   Registry activation of the five-role immutable release set. Activation is
   **one role per transaction**: whole-ELF hashing costs about one compute unit
   per two bytes, so admitting the real seven artifacts in a single transaction
   cannot fit under the chain maximum. A partially activated cache cannot
   decode, so no reader can consume a half-activated release set.

Loader-v3 owns authority presence with the tag at byte 12. Its real
`Some -> None` serialization leaves bytes 13..45 as inactive storage rather
than clearing the former key; the ELF still begins at byte 45. The runner pins
and verifies that exact retained-byte poststate, while Registry never exposes
inactive bytes as an authority.

3. Creation of a real Token-2022 collateral Mint and wallet, preserving raw
   `u64` atoms and treating the full `u8` decimals field as display metadata.
4. Bounded Registry `Begin -> Append -> Finalize` publication of the Realm,
   Runtime-V2 Product graph, Source material, recovery policy, and capability
   manifest. The Product root, result domain, and portfolio are compiled and
   published through one chain-derived graph state machine.
5. One same-slot pre-credit projection of the canonical Market and
   `Market+generation` lifecycle-rent PDA, followed by RentCreditV2 creation
   and finalized reacquisition.
6. Publication of a finalized address lookup table covering the Found frame,
   and submission of Found31 as a packet-safe v0 transaction. With its keys
   inline the canonical 31-account frame serialises to 1,242 raw bytes against
   Solana's 1,232-byte legacy limit — it misses by ten. Routing is table data,
   never protocol authority: only
   non-signer coordinates and the invoked Program are routed, the fee payer and
   every signer stay in the message's static key list, and the table is
   authority-owned rather than frozen so its rent stays recoverable.
7. Canonical Core Found31 creation from the post-credit snapshot.

It emits finalized transaction metadata, exact poststate account hashes, and
hostile observations for wrong infrastructure authority, pre-revocation
activation, late atomic rollback, substituted Registry refund wallet, a
substituted lifecycle credit in Found31, and a substituted Market coordinate
under attacker-chosen routing whose whole multi-instruction transaction must
roll back to a fee-only debit.

The runner creates every signing keypair in process memory, gives `prepare`
only the Core authority public key, and retains no private key on disk. The run
spec contains semantic market inputs—not account addresses or caller-authored
digests. The Rust compiler and chain-derived operators own every record digest,
PDA, instruction frame, and next publication action.

## What the campaign reaches: an Open Market

Measured on a real localhost validator with all seven real artifacts bound into
the release set. The campaign publishes the record graph, creates a Market
through canonical Core Found31, stages the entire projected-Custody prestate
through `DCLTPCB1` at its own generation, and then founds a *second* Market
atomically through `DCLTGMF1`.

```text
slot=1444 cu=223540   create canonical Found31 Market
slot=2021 cu=754119   create the projected-Custody founding prestate (DCLTPCB1)
slot=2630 cu=1184132  found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF1)
```

**The Market is Open.** One transaction carries five stages in one rollback
domain — Custody `LockHoardAndCloseSource`, Core Found-and-permit, Custody
`RealizeAndClose`, Claims `FoundingV5`, and Core Open **last** — at 84.6% of
Solana's 1,400,000 compute maximum:

| stage | program | CU |
|---|---|---:|
| Lock (Registry reauth 27,562; Token-2022 `TransferChecked` and `CloseAccount`) | Custody | 105,722 |
| Found and permit (Registry reauth 48,071) | Core | 414,957 |
| Realize (Registry reauth 27,562) | Custody | 87,222 |
| Claims `FoundingV5` (four Registry reauths) | Claims | 260,279 |
| Open, commit-last, plus the outer's five joins — **arithmetic**, the RPC truncated the log | Core + Trading | 315,652 |

Release-set activation is one role per transaction; the worst of the five is
Trading at 710,601 CU.

### `DCLTPCB1` is no longer heap-bound, and it never was program-side

This file used to say that the four-stage bootstrap exhausted the program heap
entering `OpenSourceCompartment` and that **no runner could fix it**, because
`RequestHeapFrame` enlarges the region the runtime grants while the stock
`solana-program-entrypoint` allocator is built with the compile-time constant
`HEAP_LENGTH = 32 * 1024`. That was true of the tree that measured it.

`9abed0c` gave Trading its own entrypoint and its own allocator, which
re-derives the grant from the instructions sysvar under agave's own
`sanitize_requested_heap_size`, and allowed exactly two routes to lift the
ceiling: `DCLTGMF1` and `DCLTPCB1`. That still did nothing, for a reason worth
recording: the adapter finds the sysvar by scanning the **top-level
instruction's own account list**, and neither route presented it. **The grant
was a wire fact, not a transaction fact.** Both routes now carry one
authenticated readonly sysvar slot in their fixed prefix, and both founding
transactions carry `ComputeBudget::RequestHeapFrame`. Frame widths moved by one:
`DCLTPCB1` 78 → 79 fixed (82 here), `DCLTGMF1` `134 + funding_count` →
`135 + funding_count` (138 here).

With the grant reaching the route, `DCLTPCB1` completes all four stages with
645,581 CU unspent. It is neither heap-bound nor compute-bound.

### Five pre-fundings, two of them exact

Nothing in the protocol funds these. Core allocates the Market and the one-shot
founding permit and Claims allocates the aggregate, the founder Position, and
the admission with `allocate` + `assign` only — **never a transfer**.

| account | requirement |
|---|---|
| Market | `lamports == rent.minimum_balance(352)` **exactly** — an over-funded Market refuses |
| one-shot permit | `lamports == rent.minimum_balance(608)` **exactly** |
| Claims aggregate | at least `rent.minimum_balance(256 + 8·claim_count)` |
| founder Position | at least `rent.minimum_balance(128 + 8·claim_count)` |
| Claims admission | at least `rent.minimum_balance(512)` |

All three Claims balances are **digest-bearing**: Core reads them at the Found
stage and folds them into the Claims request it commits to inside the permit, so
a pre-funding one lamport off does not overpay — it moves a digest and refuses
at Claims. Earlier revisions of this file said three, then four. The Found
caller-authority PDA, the fourth, needs nothing: the Market being exactly
rent-funded makes the kernel's Market rent top-up zero and the payer transfer is
skipped.

### The runner authors no digest

The founding commits to values that do not exist yet, and the campaign's
discipline survives that. The Lock and Realize receipts are produced by running
the Custody kernel's own transitions over the exact prestate bytes `DCLTPCB1`
left on chain — the `SourceFunded` projection and its normal source replay are
read back, not modelled. The permit intent and the Claims request are assembled
from the same authenticated coordinates, in the same order, that Core rebuilds
inside the Found stage. The one value that cannot be read back is the candidate
Core state the Found stage will write, whose digest the Realize receipt commits
to two stages early; every field of it is fixed by the kernel, and the encoding
is cross-checked by re-encoding the Found31 Market's own decoded state and
requiring the bytes the chain is holding.

### Final poststate

| account | owner | bytes | contents |
|---|---|---:|---|
| Market | Core | 352 | phase **`Open`**, readiness **`Consumed`**, no terminal receipt, derived identity, rent beneficiary the founding generation's credit |
| Claims aggregate | Claims | 288 | `256 + 8×4` for a four-outcome Product |
| founder Position | Claims | 160 | `128 + 8×4` |
| Claims admission | Claims | 512 | |
| Hoard vault | Token-2022 | 165 | the exact founding principal, `Initialized`, no delegate or close authority |
| normal Custody replay | Custody | 288 | realized in place from the 808-byte projection, one open vault, `next_revision == 1` |
| source vault, source replay, permit | — | — | **closed / consumed**, all three returned to the lifecycle credit |

### The hostile cases

Six, all executed against the real chain, all required to fail and to leave no
poststate behind.

- **`DCLTGMF1` refuses a substituted Claims request**, 33,594 CU,
  `TradingSbfError::Content` raised before Trading's first CPI. The substituted
  readonly record differs in exactly one coordinate — the **founder** whose
  Position and admission the founding mints — and is otherwise byte-identical.
  Lock, Found, Realize, and Claims all roll back; the runner requires a fee-only
  debit and all five allocated accounts still vacant.
- **`DCLTPCB1` refuses a reordered FundingState tail**, 685,198 CU. Earlier runs
  recorded this case as *not evidence*, because it refused within a few hundred
  CU of an out-of-memory death at the same place. It is attributable now: the
  honest transaction succeeds with an identical frame shape at 754,119, so the
  refusal is 68,921 CU short of anywhere the honest path ends.
- **`DCLTPCB1` refuses a well-formed but non-terminal request**, 22,860 CU,
  inside `decode_projected_request` before any CPI.
- plus the three Found31 and Registry cases this file already described.

### The way back out of a prestate that never founded

The campaign stages **two** prestates. Both run the identical four-stage ladder;
they differ in generation, in how long their founding stays satisfiable, and in
which of the two exits out of `SourceFunded` they take. The second exists to be
abandoned.

`OpenSourceCompartment` puts real collateral under a projected authority against
a Market that does not exist, and until `d43536d` nothing accepted the phase it
left behind. The forward direction is an atomic founding whose Core Found and
Open stages both refuse an expired artifact, so past `expiry_slot` that
collateral could not be moved in any direction by any route.

```text
slot=3083 cu=765807  stage a second projected-Custody prestate for the expiry abort (DCLTPCB1)
slot=3180 cu=134666  DCLTPCA1 refuses to abort a funded source before expiry     REFUSED
slot=3599 cu=148996  unwind an expired founding's funded source compartment (DCLTPCA1)
```

The refusal is the half that matters: while the founding is still satisfiable
the authority over funded principal may not be destroyed. It refuses with
`CustodySbfError::Expiry`, rolls the whole multi-instruction transaction back to
a fee-only debit, and the runner then re-authenticates the entire prestate to
prove it moved nothing. The honest abort, 419 slots later, returns exactly the
principal to the party that supplied it, closes the source vault, the source
replay, the empty Hoard vault, and the projection, and raises the lifecycle
credit by exactly those four rents.

The abort lane's expiry is squeezed from both sides — `initialize` refuses once
`current_slot > expiry_slot`, so it must outlast staging, and every slot past
that is dead waiting. The runner does not trust the arithmetic: it checks the
remaining margin before staging and waits for the real slot afterwards.

### What the frame forced

- The founding runs at **its own generation**: every projected-Custody stage
  asserts the inverse of a live Market, so the Found31 Market cannot be reused.
- The **principal supplier is not the rent payer**, and the beneficiary is named
  once — Custody requires the principal's owner to sign while non-writable and
  the payer to be writable, and privileges are per key. The same key is the
  founding artifact's `beneficiary`, the Lock's `refund_owner`, the owner of the
  Token-2022 source wallet, and the lifecycle credit's refund wallet.
- **No account in the `DCLTGMF1` frame is a transaction-level signer**, so the
  fee payer is the one key that appears nowhere in it. **65 distinct keys**,
  eleven of them writable, against agave's 128-account lock limit.
- Privileges are **unioned per key** before sending. The Market is the clearest
  case: Custody's Lock stage requires it non-writable and vacant, and Core's
  Found stage creates it.
- `context_digest = sha256(b"dclutch:projected-hoard-context:v1" || context)`
  while `funding_source_context` is that context **undigested**; both are
  caller-PDA seed inputs, so a wrong one produces an address for which no
  signature exists.
- The Market identity the campaign carries has a **placeholder `market_id`**,
  because `market_id` is not one of the nine Market-address seeds. That is
  harmless until a founding has to commit to the digest of the Core state the
  Found stage will write.

Two earlier defects on this path were fixed at their semantic owners: the host
Found/RentV2 projections refused the real System Program (`c25de27`), and the
capability-root selection was a SHA-256 fixed point and so unsatisfiable for
every well-formed artifact (`386f254`). The full decomposition, the per-role
activation table, the artifact digests, and the complete transcript are in
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`.
