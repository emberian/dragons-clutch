# Concrete source-profile dossier: Pyth boundary prices

Status: **PROPOSED semantic profile; STOP on production promotion**

Review date: **2026-08-18**

## 1. Decision

Pyth Core is the only reviewed candidate that exposes a documented relation
capable of removing submitter choice for an objective point price.  Its signed
`PriceFeedMessage` says the unique update for instant `T` is the update for
which `prev_publish_time < T <= publish_time`.  The historical endpoint returns
the first update with `publish_time >= T`.  A Dragon's Clutch bucket can
therefore freeze one boundary instant and admit only the crossing message.

This closes **value selection**, not the entire source adapter.  No candidate is
ready to be called production-qualified today.  Pyth remains the leading
conditional profile because the remaining gaps are identifiable Solana adapter
and availability gaps rather than an absent canonical-selection rule.

| Candidate | Authenticated value | Canonical value at frozen boundary | Historical proof | Permissionless advance | Verdict |
| --- | --- | --- | --- | --- | --- |
| Pyth pull + Benchmarks | quorum-signed update verified by receiver | yes: `prev < T <= publish` | signed payload fetched from Benchmarks/Hermes | any payer can post, but API credential and fees apply | **conditional prototype** |
| Pyth current/push account | receiver-owned current value | relation exists in message | mutable latest account overwrites history | any updater can race the account past a boundary | reject as sole archive |
| Switchboard OracleQuote | oracle signatures tied to a recent Solana slot hash | no; several fresh quotes/slots can qualify | canonical quote account stores mutable latest data | managed updater is permissionless in transaction construction | reject for V1 settlement |
| Orca Whirlpool state | DEX program-owned pool state | no; keeper chooses a sample transaction | no protocol-native price history in reviewed state | reading is permissionless, canonical observation is not | reject for V1 settlement |

## 2. Exact proposed semantic rule

For an immutable grid origin `G`, duration `D`, and cursor `k`, define the
right-hand boundary:

```text
T(k) = G + (k + 1) * D.
```

The record for bucket `k` is the fully verified message for the frozen feed ID
whose timestamps satisfy:

```text
prev_publish_time < T(k) <= publish_time.
```

Additional admission checks are mandatory:

- `publish_time` and `prev_publish_time` are nonnegative;
- a message with `prev_publish_time == publish_time` selects no boundary;
- the signed feed ID and immutable base/quote orientation match;
- price is positive;
- confidence is widened by the immutable multiplier, capped absolutely and
  relatively, and normalized with outward rounding;
- receiver account owner, account discriminator, exact layout, and
  `VerificationLevel::Full` match the pinned release;
- `posted_slot <= Clock.slot` and its age is bounded;
- `Clock.unix_timestamp` has passed `T(k)` plus an immutable grace period;
- Pyth publish time is not unreasonably ahead of the canonical Clock; and
- the exact accepted interval is copied into a Dragon's Clutch program-owned
  append-only archive before the caller-controlled update account can change or
  close.

The price is a boundary sample, not a TWAP and not a claim about every trade in
the bucket.  Terms and UI must say so.

### Why this removes caller choice

A merely fresh update admits many prices.  The crossing predicate names the
single update whose adjacent publish times straddle the already-frozen `T(k)`.
The caller may submit that update or fail to advance; it cannot substitute a
later profitable update without violating `prev_publish_time < T(k)`.

This conclusion depends on Pyth's signed-message semantics.  It does not prove
honesty of the Pyth signer quorum.  The upstream definition also warns that a
crossing message can be absent during message-delivery migrations and that an
unsuccessful aggregation can set equal timestamps.  Dragon's Clutch must stall,
not manufacture `Missing`, in both cases.

## 3. Pyth trust and deployment analysis

### 3.1 Signer trust

The reviewed upgrade documentation describes five independent Pyth Pro routers
and a 3-of-5 quorum.  The legacy path used a 13-of-19 Wormhole guardian quorum.
`VerificationLevel::Full` means the receiver accepted the applicable full
quorum; it does not make the data trustless with respect to that quorum or its
upstream exchanges.

The confidence interval is useful, but it is not proof against a quorum-wide
bad aggregate.  Immutable terms should state the Pyth feed, signer architecture,
confidence multiplier/caps, and the consequence of source failure.

### 3.2 Upgrade identity is not just a program address

The official source currently contains two receiver identities:

- legacy/default `rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ`; and
- pro-compatible `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp`.

The same source tree's current upgrade guide says the upgraded cutover is
2026-08-26 and calls for the `pro-compatible` SDK feature.  The SDK manifest at
the reviewed commit reports version 2.0.0 while one guide snippet still says
1.2.0.  That is an active-transition signal, not a fact to smooth over.

A deployable profile must pin and recheck:

1. receiver program ID;
2. upgradeable-loader ProgramData address;
3. ProgramData deployment slot/generation;
4. receiver `Config` PDA key and canonical byte digest;
5. accepted data-source/signer configuration; and
6. exact parser/adapter digest.

Pinning only the program deployment is insufficient.  The reviewed receiver
has governance instructions that mutate valid data sources, fees, Wormhole
address, and minimum signatures without upgrading the program.

### 3.3 Why an arbitrary receiver-owned account is insufficient

`PriceUpdateV2` is a caller-created account.  Its write authority may update it
or reclaim rent, and the account does not record the receiver `Config` digest
under which it was posted.  A hostile governance sequence could loosen config,
post a receiver-owned account, and restore config.  Checking only current config
and account owner later would accept ambiguous provenance.

The narrow promotion path is an atomic join:

- inspect the Instructions sysvar and require the immediately preceding
  instruction to be the exact pinned receiver post instruction with this update
  account and pinned config, **or** CPI the pinned receiver from the Clutch
  instruction;
- recheck ProgramData and receiver Config in the consuming instruction;
- parse and archive the update immediately; and
- cover set/post/restore, stale-account reuse, write-authority reuse, wrong CPI,
  and same-slot substitution with hostile SVM tests.

The boolean statement “posted immediately before” is intentionally not modeled
as proof in this crate.  It belongs in the Solana adapter.

## 4. History, availability, and operatorlessness

Pyth Benchmarks/Hermes is an offchain historical delivery service.  It returns
signed update payloads that can be checked onchain; the HTTP response itself is
not trusted.  The official endpoint documents “first update whose publish time
is at least the requested timestamp,” which pairs with the signed
`prev_publish_time` crossing check.

This provides integrity but not permanent availability:

- no reviewed Solana account retains all historical crossing messages;
- the current documentation requires a Pyth API key after the announced
  cutover;
- a shared API secret cannot safely be embedded in GitHub Pages/IPFS; and
- old signer-generation verification material may have finite practical
  availability, so late recovery needs an explicit tested horizon.

An ordinary user or keeper can advance without Dragon's Clutch infrastructure
by supplying its own Pyth API credential, RPC, fee payer, and signed update.
Anyone may also mirror signed update blobs because verification is onchain.
That is operatorless with respect to us, but it is not anonymous, costless, or
independent of Pyth's historical service.

Once accepted, the normalized record must live in the Clutch archive.  Later
resolution consumes only that sealed archive.  If nobody submits the crossing
record within the supported verification/availability horizon, the feed stalls.
Liveness incentives may use explicit fees or external bounties; Hoard principal
must never fund them.

## 5. Clock and finality

The SBF program reads the canonical Clock sysvar.  Instruction data may not
supply slot or Unix time.  Clock gates “the boundary has passed,” update-account
posting age, and bounded source-time skew.

Solana finality is a client/ledger property, not a field a running instruction
can demand of its own bank.  A successful instruction creates an archive record
on the executing fork; clients and keepers should wait for `finalized` RPC
commitment before treating that record as operationally final.  Consensus state
eventually selects the surviving fork.  A protocol rule should not pretend that
an in-instruction Clock read proves RPC finality.

Pyth `posted_slot` is the Solana receiver-write slot.  It is useful for freshness
and monotonicity after archiving, but it is not a source-native price sequence.
Pyth `PriceFeedMessage` does not expose the `publish_slot` found on its separate
TWAP message type.

## 6. Why the other candidates fail the canonical-history test

### Switchboard OracleQuote

The reviewed quote authenticates a signed Solana slot hash, a recent slot, feed
ID, fixed-point value, and minimum sample count.  The verifier checks the slot
against the SlotHashes sysvar and a maximum age.  The reviewed feed payload has
no previous-publish link, wall-clock publish time, or confidence interval.

Thus two quotes from two recent slots can both be valid for the same Clutch
bucket.  A deterministic canonical quote account address prevents account-name
substitution but not value selection, and its mutable latest state is not a
historical proof.  Switchboard is a useful cross-check or live risk input; it is
not this V1 settlement source.

### Orca Whirlpool / direct DEX state

The reviewed Whirlpool account stores current `sqrt_price` and current tick.
The separately named `Oracle` account in the current Orca program is adaptive
fee state, not a historical price oracle.  Neither gives a program-verifiable
“first observation after T” record.

A keeper reading a pool at any time chooses the sample.  A trader can also move
the pool state, so a DEX profile needs explicit liquidity/manipulation analysis
even after history is solved.  A new observation accumulator cannot observe
every Whirlpool swap unless the DEX program itself writes/calls it; periodic
permissionless sampling merely relocates the selection problem.

## 7. Required changes before runtime promotion

1. Wait through the announced Pyth Core cutover; pin post-cutover official
   program IDs, ProgramData deployment slots, source code/build evidence, and
   receiver Config bytes.
2. Decide whether the protocol accepts Pyth's 3-of-5 router trust model.
3. Revise `SourceSpecV1`: represent a pull-profile program/config identity and
   ephemeral update accounts instead of one exact immutable data-account key.
4. Revise the selection registry from `publish_time / bucket_seconds` to an
   origin-bound crossing rule `prev < T(k) <= publish`.
5. Do not require a nonexistent Pyth source publish slot.  Give posted slot,
   publish time, and archive sequence distinct semantic owners.
6. Either let parsers return exact low/high endpoints or specify the extra
   widening needed to encode outward decimal rounding as center/radius.
7. Implement immediate receiver-post/CPI provenance and Instructions-sysvar
   checks with exact account metas and config digest.
8. Add official post-cutover receiver fixtures, loader/config fixtures, expected
   red mutations, SVM tests, compute/transaction-size measurements, and a
   historical-recovery-horizon test.
9. Bind `Resolve` only to the sealed program-owned authenticated archive.

Until all nine close, the correct runtime behavior is refusal.
