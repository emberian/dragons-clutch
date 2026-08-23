# ADR-0008: individually funded candidate admission

Status: **experimental pure successor; no live tags or SBF route**

## Context

ADR-0006's executable V2 kernel pre-creates four epoch-sponsored index pages.
`BeginCandidateV2` consumes one of 64 shared slots before a feed is sealed. This
bounds account growth, but it also creates a quality denial: sufficiently many
cheap, low-quality or abandoned beginnings can exclude a later, better
candidate even when that candidate would fund all of its own work. Increasing
the fixed cap only moves the attack threshold and makes the sponsor capitalize
more rent.

V2 also binds a solver and reward destination without specifying a
copy-resistant admission ceremony. A searcher who first learns a public witness
can attempt to admit the same witness with its own reward destination. Binding
the destination after the witness is public does not prevent that simple copy.

The two half-open windows remain useful and are not the problem:

```text
[F, S) submission
[S, V) verification
[V, +infinity) hard terminal path
```

The successor must preserve those boundaries and selection of the best valid
submitted candidate under one checked total rank. It must also make every live
child enumerable and retirement counted without asking an epoch sponsor to buy
a finite shared candidate page.

## Decision

Introduce a clearly named, registry-independent successor kernel:

- `CandidateAdmissionPolicyV3`;
- `CandidateWindowV4`; and
- one `CandidateAdmissionNodeV3` per admission.

These are host-side semantic names, not globally allocated account versions.
No V2 codec byte, local envelope, or intent changes meaning. A future registry
wave must assign fresh collision-checked tags and account versions.

### Submission has commit and reveal subintervals

Freeze stamps four checked boundaries:

```text
R = F + commit_span_slots
S = R + reveal_span_slots
V = S + verification_span_slots

[F, R) commit
[R, S) reveal
[S, V) verify
[V, +infinity) terminal
```

Commit and reveal are both parts of the original half-open submission window.
At `R`, commits refuse and reveals open. At `S`, reveals refuse and verification
opens. At `V`, verdict mutations refuse and hard finalization opens. Early
finalization after `S` is allowed only after every admission has a terminal
verdict or expiry.

The adapter verifies a commitment with the exact domain
`dragons-clutch/candidate-commitment/v1` over:

```text
epoch
admission_policy_id
submitter_authority
solver_reward_destination
candidate_digest
secret
```

The kernel persists the commitment, submitter, and reward destination at
commit, then accepts only an adapter-attested opening with all fields equal.
Because commits have closed before any opening is accepted, learning a witness
from a reveal does not let a copier create a new commitment for that witness.
Replaying the reveal can only advance the original node whose reward destination
was already bound. Independently discovering and precommitting the same
economic candidate remains allowed.

This is a narrow copy-resistance claim. It does not hide reveal transactions,
prevent a block producer from censoring them, solve general MEV, or prove that
the future adapter hashes the stated preimage correctly.

### Each admission buys its own node

There is no policy candidate-count cap and no sponsor-funded index page. One
successful commit atomically:

1. proves a fresh canonical node identity;
2. sets `ordinal = admitted_count + 1` with checked arithmetic;
3. records the prior Window head in `previous_node`;
4. makes the new node the Window head;
5. increments `admitted_count` and `live_node_count`; and
6. collects exactly the node rent principal, admission bond, and node cleanup
   reward from that admission's payer.

The epoch sponsor does not fund or own any admission-page rent. The finite
commit interval and chain transaction throughput bound successful creations in
one epoch. A spammer can buy many nodes and contend for transaction or later
verification bandwidth, but cannot consume a small, shared protocol slot that
would categorically exclude the next candidate. Every admitted node carries
the reward needed for its own permissionless close. An unrevealed commitment
also pays the immutable abandonment penalty from its own bond.

This removes the fixed-capacity quality denial. It does not prove that
verification bandwidth is economically uncontentious; measurements and
candidate work pricing remain a promotion gate.

### Enumeration and retirement are reverse-linked and counted

The Window owns:

```text
admission_head
admitted_count
live_node_count
closed_node_count
```

Every node owns its immutable `previous_node` and `ordinal`. Creation is one
node per instruction. After finalization, cleanup is also one node per
instruction and may close only the authenticated head with
`node.ordinal == live_node_count`. A successful close replaces the head with
the node's predecessor, decrements `live_node_count`, and increments
`closed_node_count`.

The invariant is:

```text
live_node_count + closed_node_count = admitted_count
live_node_count = 0 iff admission_head = zero
```

Thus a caller never supplies an alleged exhaustive list, and no instruction
has work proportional to the candidate count. Older nodes can be delayed by a
newer live node, but after `V` every unfinished state has a permissionless,
prepaid expiry path. A future counted Epoch must increment its authoritative
candidate-node counter on creation and decrement it in the same transaction as
node deletion. The pure kernel exposes that as an adapter obligation; it does
not claim the current Epoch supports retirement.

Node close requires adapter-authenticated closure of the corresponding
candidate/work bundle. If the node is selected, it additionally requires
settlement-terminal evidence. Rent principal and the refundable bond return to
the recorded refund destination, the cleanup reward goes to the closer, and
the abandonment penalty plus unsolicited surplus go to the immutable neutral
sink. Hoard principal, collateral, future fees, and future trading revenue fund
none of these compartments.

### Selection remains deterministic

A checked valid verdict supplies the rank under the Window's immutable score
policy. The last 32 active rank bytes remain the complement of the admission
node identity, so otherwise equal economic scores have an injective canonical
tie break independent of append order. Each valid verdict compares against the
persisted best rank and replaces it only when strictly greater. Finalization
selects that identity once.

The result is the **best valid submitted candidate**. It is not optimal
clearing unless a separately checked optimality certificate exists.

## Invariants

For every state reachable through the pure transitions:

```text
revealed + expired_commitment <= admitted
verdict + expired_unverified <= revealed
valid_verdict <= verdict
live_node + closed_node = admitted
```

A node moves only through one of:

```text
COMMITTED -> REVEALED -> VERIFIED_VALID
                     \-> VERIFIED_REFUSED
                     \-> EXPIRED_UNVERIFIED
          \-> EXPIRED_COMMITMENT
```

Commit, reveal, verdict, both expiry transitions, finalization, and close are
error-atomic over copied values. Replays refuse. Arithmetic uses checked exact
integers. No numeric cast, allocation, float, unsafe code, SDK type, CPI, or
account-memory operation appears in the successor kernel.

## Consequences and remaining gates

Positive consequences:

- a candidate no longer loses admission because 64 shared slots were bought by
  earlier low-quality beginnings;
- the party creating a node funds its rent, bond, and eventual cleanup;
- post-reveal witness copying cannot redirect the committed reward destination;
- append order does not decide equal economic-score selection; and
- enumeration and deletion remain exact, resumable, and one-node bounded.

Costs and open gates:

- commit/reveal adds one boundary and at least one transaction per candidate;
- all commits and verdict updates still serialize on the Window head/best cache;
- reverse cleanup can delay an older node until all newer nodes terminalize;
- commitment hashing, canonical PDA derivation, signature/account authority,
  funding derivation, candidate-bundle joins, and lamport movement remain in an
  unimplemented adapter trust boundary;
- no live global tags, intents, SBF routes, linked ELF, CU/rent measurements, or
  local-validator campaign exist for this family; and
- verification-bandwidth contention, proposer censorship, and general MEV are
  explicitly not solved.

## Rejected alternatives

- **Raise or dynamically tune the shared cap.** This retains a categorical
  crowd-out threshold and shared sponsor rent.
- **Evict the current worst candidate.** Eviction makes rent ownership and live
  child cleanup ambiguous, and a later verdict can change which candidate was
  safe to evict.
- **Use unlinked candidate accounts plus an offchain index.** A static index is
  an untrusted projection and cannot prove exhaustive retirement onchain.
- **Commit and reveal throughout the same interval.** Once one reveal is public,
  a later commit can copy it with a new reward destination.
- **Claim commit/reveal solves MEV.** It only blocks the simple reward-copy path
  described above.

## Evidence

The host adversarial suite covers:

- 80 individually funded admissions, exceeding the V2 cap of 64;
- exact `F/R/S/V` boundary behavior;
- copied openings, reward substitution, and reveal/verdict replay;
- two identical candidate digests with equal economic score and canonical
  node-identity tie breaking;
- abandoned commitment penalty/refund conservation;
- hard finalization with unfinished work;
- reverse-order refusal, one-node cleanup, underfunding, surplus routing, and
  selected settlement evidence; and
- final `live + closed = admitted` retirement state.

Run:

```sh
cargo test --release --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo clippy --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml --no-deps
```

This is host kernel evidence only. It is not SBF, linked-program, local
validator, devnet, mainnet, audit, or formal-verification evidence.
