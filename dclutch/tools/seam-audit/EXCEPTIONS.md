# Seam-audit verdicts

Every entry in `baseline.json` carries a verdict tag, and every tag in use must
have a section here. The gate enforces that: a tag with no written reason is
reported as `UNREASONED` and fails, so an exception cannot be accepted by
editing JSON alone.

A verdict is a claim about a finding, and the claims are not interchangeable.
`benign-*` says the finding is not a defect. `debt-*` and `hazard-*` say it
**is** one, of a known shape, not fixed in this lane — recorded so it cannot be
mistaken for clean and so the ratchet stops the population growing.
`inventory-*` is not a finding at all. Naming debt as debt is the point:
recording a hazard is not the same as closing it, and a register that blurs the
two is worse than no register.

Baseline written 2026-08-29, 634 entries; re-verified clean at `05372c0f`.
Rewritten 2026-08-30 at `fd8cad39`, 728 entries, and from that date the register
carries the revision it was measured at in its own `measured_commit` field —
`--write` reads a committed tree and cannot read the working one, because this
is a shared checkout and an unfinished file looks exactly like a finished one to
a static reader.

Main still moves under this file. What the baseline pins is the finding *set*:
if it still reproduces exactly, the gate is green wherever main is. The commit
is recorded so the set can be reproduced later, not to bind the gate to it.

---

### confirmed-defect

**1 entry. An open defect, reachable, not fixed here.** It is recorded
so the gate does not report the tree as clean while it stands — not excused.

**Was four until 2026-09-03**, and the other three were the Claims founding
byte collision and its two `DOMAIN_NAME_BYTES_DISAGREE` companions. See the
2026-09-03 SEAM entry at the bottom of this file: `b209be565` gave
`LiabilityBasisV2` a Lean owner and made
`CLAIMS_FOUNDING_AGGREGATE_SEED_V4`/`_V5` **aliases** of
`LIABILITY_BASIS_MARKET_SEED_V2` rather than second literals, which is the
answer this section demanded — *someone who owns Claims founding has to say
which it is* — written in the source.

Was five. `PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2` +
`RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2` — the two-crates-one-byte-string
collision described at the bottom of this section — **no longer reproduces at
`fd8cad39`**, along with its `DOMAIN_RAW_RESTATEMENT` and
`SEED_DOMAIN_UNASSERTED` companions in the Rational family. Somebody fixed it
between `05372c0f` and here without the register being told, which is exactly
what the `GONE` half of the ratchet is for: a fixed defect has to *leave*, or it
stands as cover for the next one. Its entry below is kept as history and marked.

- `TRANSACTION_LEVEL_SIGNER_CENSUS` ·
  `programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs` ·
  `authenticate_expired_checkpoint_v1` — this is SEAM_AUDIT #13b, and the
  checker found it unaided. The frame refuses `is_signer` on every account,
  but `is_signer` is a *transaction-level* property: the fee payer is message
  key 0 and reads true in every instruction that names it, whatever
  `AccountMeta` it was given. Since `5ca145e8` the funding source **is** the
  campaign payer and the builder places it at `FUNDING_ABORT_FUNDING_SOURCE = 7`
  (`tools/local-validator/bootstrap/successor/src/market.rs:7505`) in
  transactions that payer signs. All three abort routes are gated, so an
  expired founding can never be unwound: principal stays in the Custody source
  vault and rent stays in two ledgers plus the checkpoint, permanently. Owner:
  Trading. Probe already written in `SEAM_AUDIT_2026_08_29.md:790-793`.

- `DOMAIN_BYTES_COLLIDE` · `CLAIMS_FOUNDING_AGGREGATE_SEED_V4` +
  `CLAIMS_FOUNDING_AGGREGATE_SEED_V5` + `LIABILITY_BASIS_MARKET_SEED_V2` +
  `CLAIMS_MARKET_SEED_V2` — four names, one byte string
  `dclutch:lbv2:market`, across `crates/dclutch-claims` and three
  fixtures. The sharp part is V4 beside V5: they are byte-identical, so the
  version bump lives in the *name* and not in the address, and both versions
  derive the same PDA. Their neighbouring constants do differ by version
  (`CLAIMS_FOUNDING_WIRE_VERSION_V4 = 4` vs `..._V5 = 5`), which makes the
  un-bumped seed read as an oversight rather than a decision. Someone who owns
  Claims founding has to say which it is.

- `DOMAIN_NAME_BYTES_DISAGREE` · the same two constants — the name claims
  `claims/founding/aggregate` and the bytes carry `dclutch:lbv2:market`,
  sharing no segment at all. Filed separately because it is a separate fact: a
  reviewer trusts the identifier and the chain sees the literal, and here they
  describe different things.

- **FIXED, left the register at `fd8cad39`.** `DOMAIN_BYTES_COLLIDE` ·
  `PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2` +
  `RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2` — two crates, one byte string
  `dclutch:rational-claims:v2`, and two *different stated meanings*: one
  doc-comment says "canonical Claims custody owner", the other says "rational
  capability owner". Neither references the other and neither is guarded. If
  both meanings are real the address space is shared by two authors who do not
  know it; if only one is, the other is dead. This is the class in its purest
  form and is exactly why the reader matches on bytes rather than identifiers.

### benign-distinct-programs

**1 entry.** `GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1` and
`GENERAL_ACCELERATOR_CALLER_AUTHORITY_SEED_V1` carry the same bytes
(`general-accelerator-test-caller`) but are derived under *different program
ids* — one is the accelerator's test caller program, the other the bootstrap's
campaign. A PDA is a function of both the seeds and the program, so identical
domains under different programs are different address spaces and cannot
collide. Recorded rather than suppressed so the reader stays honest about what
it matched.

### benign-abbreviation

**1 entry.** `STAGING_CURSOR_PDA_SEED_V1` carries `dclutch-record-stage-v1`.
"staging" against "stage" is abbreviation, and "cursor" is genuinely absent
from the literal, but the constant has one author
(`crates/dclutch-registry`), a compile-time length guard, and 107
consistent three-seed derivation sites. The name/bytes gap here is stylistic.

### debt-seed-guard

**62 entries.** A PDA seed domain within 32 bytes today with no compile-time
assert holding it there.

Three of them are **assigned, not accepted**: `CLAIM_CHECK_SEED_V1`,
`CLAIM_CHECK_ESCROW_SEED_V1` and `CLAIM_CHECK_VAULT_SEED_V1` in
`crates/dclutch-claims`, which arrived with the claim-check work and are the
ratchet's first live catch. They are one line each and they belong to whoever
owns that crate. They are recorded rather than fixed here for a reason that is
itself the subject of this tool: at the time, that file carried another lane's
uncommitted changes in a shared checkout, so editing it would have staged
somebody else's half-finished work under this lane's commit — the same hazard
`--write` was changed to make impossible. Closing it in the tool and then doing
it by hand would have been the wrong trade. Not a live defect — every one of these derives an
address right now — but the audit's own closing recommendation is to finish this
coverage, because the two guards that actually caught things in that sweep were
this one and a frame-width tie, and where they exist they work. The fix is one
line per domain and the guard is free at runtime.

The population is real debt, and the ratchet is what makes it shrink: a *new*
unguarded domain fails the gate, and every guard added must be re-written into
the baseline, so the number can only go down. Worth naming: the 2026-08-29
audit put this at 51 and I read 61, because its `assert!(` grep missed the block
form `const _: () = { assert!(a); assert!(b); };` in two crates — the count moved
in both directions once the reader saw all five guard dialects.

### debt-derivation-restatement

**241 entries.** A seed tuple spelled out in a crate that does not own the
domain, where the owning crate exports a seed constructor for exactly that
purpose. Each is a second author for one address.

The population moved in both directions on 2026-08-30, and the directions mean
different things. **Two left because they were fixed**: the Direct trade
producer's raw-record and staging-cursor derivations now go through
`RecordKeyV1`, which is what the ratchet is for — a *new* file restating an
existing domain should be corrected, not filed beside the 239. **Three arrived
and are assigned**: `claim_check_compaction_v1.rs` in `dclutch-claims-sbf`
restates the two Record domains and `LIABILITY_BASIS_MARKET_SEED_V2`. Same
owner and same reason as the `debt-seed-guard` three above — that file was
carrying another lane's uncommitted work. Worth flagging to whoever takes them:
`LIABILITY_BASIS_MARKET_SEED_V2` is one of the four names in the confirmed
`DOMAIN_BYTES_COLLIDE` above, so a second author for *that* domain is a second
author for a byte string whose first author is already in dispute.

These are not equally bad and the register does not pretend otherwise. Some are
forced: `create_program_address` must append the bump, and no `as_slices()`
returns a tuple with one, so all 40 of those sites re-spell by construction —
the compliant idiom there is destructuring from an `as_slices()` binding, which
64 sites already use. Others are genuine drift of the kind that produced
SEAM_AUDIT #3 and the dealer-batch defect, where the second author disagreed
with the first and no test could tell.

Triaging all 239 individually is a lane of its own. What this baseline buys
today is that the population cannot grow: a *new* file restating an existing
domain fails the gate, which is the condition under which both of today's
class-2 defects were introduced.

### hazard-signer-census

**24 entries.** A blanket `is_signer` refusal over a whole account frame,
minus the one already confirmed as #13b.

Was 28. Four left on 2026-08-30 because the reader was fixed, not because the
code changed, and the distinction matters enough to write down.

Three — `authenticate_generic_market_open_frame_v1`,
`authenticate_generic_market_founding_lock_census_v3` and
`authenticate_generic_found_and_permit_lock_census_v3` in `market.rs` — pair the
census with an explicit `meta.pubkey == payer` exclusion. That is this class's
harm statement in negated form: a frame that refuses the fee payer being *named*
in it cannot be "dead for any builder that pays with an account it also names".
The writability half has honoured an in-place exemption since it was written;
the signer half honouring none was a fact about the reader. The fourth,
`authenticate_lookup_infrastructure_planned_journal_v1`, was never a refusal at
all — both `is_signer` reads classify the coordinate into
`TerminalAddressClassV1::InlineSigner` and no `Err` is reachable from either.

**The bigger narrowing was considered and refused.** Nine of twelve sampled
sites census `AccountMeta` the builder authored rather than runtime
`AccountInfo`, and `is_signer` is only a transaction-level property for the
latter — so it is tempting to drop the whole authored-meta population. The
discriminator would have to be textual (`.pubkey` for a meta, `.key` for an
account), and `project_manifest_document_v3` is the counterexample that kills
it: `ObservedAccountMetaV3` is an authored meta type that reads
`.account.key`. Trading a bounded false-positive rate for an unbounded
false-negative rate is a bad trade in the one class that found a real
always-refuses defect unaided. Do not retry it without type resolution.

What has *not* changed is the question these 24 still need, and it is the same
one that answered the four above: does any builder of this route place a signing
account in this frame? #13b is what the answer looks like when it is yes.

The principle is not in doubt — `DEALER_ACCEPTED_TRANSITION_2026_08_29.md`
states it plainly: *an exact-privilege census is a constraint on the whole
transaction, not on your instruction.* Solana merges account privileges across
the instructions of one transaction, and the fee payer signs all of them. What
is not established for these 28 is whether any live builder puts the payer in
that particular frame, and that is a per-route reading this lane did not do.

So: hazard, not confirmed defect, and deliberately not "accepted". Each needs
someone to check one thing — does any builder of this route place a signing
account in this frame — and #13b is what the answer looks like when it is yes.

### hazard-privilege-pin

**37 entries.** An exact writability census over every coordinate of a frame
with no exemption anywhere.

Was 24, and the thirteen that arrived on 2026-08-30 were **not new code**. The
reader used to `continue` after reporting a signer census, so a function could
only ever be reported for one half of class 6 — twelve sites had a pin finding
hidden behind a signer finding, including `authenticate_expired_checkpoint_v1`,
which is #13b's own function. A gate that reports one defect per function hides
the second behind the first, which is this tool's entire subject matter, so the
`continue` is gone and the hidden twelve are now visible. The thirteenth is
`project_manifest_document_v3`, which surfaced here the moment it stopped being
reported as a signer census.

They are recorded as hazards rather than triaged because the per-route reading
below has not been done for them either. One tempting follow-up, named so it is
measured rather than guessed: several of the thirteen express a *per-coordinate*
writability expectation (`let expected_writable = index == 1`, `matches!(index,
..)`) which is arguably the same one-coordinate exemption `16351a13` was fixed
with. Recognising that dialect would drop some of these — but it must be run
against the `16351a13` historical control first, because that control's whole
job is to still catch the Custody pin, and a dialect too generous silences it.

Same principle, the writability half, and it has already bitten once:
`16351a13` found Custody pinning the checkpoint readonly while its documented
atomic partner, Trading's ingest, must take it writable — so the pin was never
a constraint on Custody's own instruction, and it made the only shape in which
a reservation can be produced and joined unsubmittable. The fix was not a
relaxation but a one-coordinate exemption, reasoned in place.

An unexempted census is not wrong by itself; most frames have no atomic
partner. It is wrong exactly when the instruction is half of a documented pair,
and that is the question each of these 24 needs asked of it. Note that
`require_activation_frame` in `dealer_reservation_v1.rs` is still on this list
after `16351a13`, which exempted its sibling `require_frame` only.

### hazard-unset-pin

**18 entries.** A frame that pins the System program by key and authenticates
a coordinate against a wire-supplied pubkey, with nothing in the function
refusing the all-zero one.

This is the forward-looking half of the class with no 2026-08-29 defect behind
it, and the honest reading is that these are mostly fine: an unset coordinate
usually fails a later derivation or ownership check anyway. It is recorded
because "fails somewhere downstream" is an argument, not a guard, and the
downstream check is one refactor away from moving.

### inventory-guard-present

**260 entries. Not findings.** Each records that one file refuses the unset
pubkey somewhere in it. Three arrived on 2026-08-30 —
`bearer-v2-operator/open_release_v1.rs`, `claims-svm/claim_check_v1.rs`,
`fractional-claim-kernel/selection_config_v1.rs` — which means three new files
grew a guard, not that three defects appeared. The class has no defect behind it, so the only useful
property to assert is that the existing guards stay: the gate's ratchet turns
both ways, so a file quietly losing its last guard is reported as `GONE` and
fails.

Keyed by file rather than by function on purpose. Function-level keys
inventoried 586 guards and would have failed the gate twice — once `GONE`, once
`NEW` — on every rename of a guarded function. A ratchet nobody can live with
gets switched off, and then it guards nothing.

### checked-caller-excludes-payer

**2 entries.** A class-6 census whose standing question — *does any builder of
this route place a signing account in this frame?* — has been asked, answered
YES, and closed in the builder, where the reader cannot see it.

`hazard-signer-census` and `hazard-privilege-pin` both mean *nobody has looked
yet*. This tag means someone did, and the looking changed the code. It is not an
acceptance: the finding is real and the pin is genuinely transaction-level. It
records that the harm this class names has already been made unreachable at the
only place it could occur.

The bar for using it is the one the two hazard notes set, and it is three parts.
The route's builders must be enumerated, not assumed. Some builder must actually
be able to put a privileged account in the frame — if none can, the entry is a
false positive and belongs in a `benign-` tag instead. And the exclusion must
exist in code and be held by a test, named here.

The reason it is a tag rather than a departure from the register: the three
`market.rs` censuses that left on 2026-08-30 paired their census with a
`meta.pubkey == payer` exclusion **in the same function**, so the reader could
see both halves. An operator-side builder cannot do that. The census lives in
the crate that authors the frame and the payer is chosen in the crate that
sends it, so the two halves are always separated by a crate boundary and no
proximity-based reader will ever pair them. Departure would require the reader
to resolve callers across crates, which is the type resolution the
`hazard-signer-census` note already refuses to fake.

---

---

## 2026-08-31 — cohort-9 CLOSEMAKER: six entries, four tags, no new class

`direct_close_maker_v1` (the maker-replay close, wall 22's missing decrement)
deliberately mirrors `direct_begin_retiring_v1`'s frame machinery, and it
inherits that family's registered debts with it, entry for entry:

- **debt-derivation-restatement ×2** (`RAW_RECORD_PDA_SEED_V1`,
  `STAGING_CURSOR_PDA_SEED_V1` in the new route): the same
  `authenticate_finalized_record`/`authenticate_persisted_raw` spelling the
  begin-retiring and fee-settlement routes carry. The real fix is one shared
  helper for all three routes — a refactor across a shipped route, not this
  lane's cut. The population grows by exactly the two the sibling already has.
- **hazard-privilege-pin ×1**, **hazard-signer-census ×1** (`parse`): the
  exact-privilege and no-signer frame is the begin-retiring lifecycle shape,
  registered for that route with the same tags. Both routes are standalone
  permissionless cranks whose fee payer is never a frame member; the batching
  cost the tags record is real and shared, and any fix belongs to the family,
  not to one member.
- **inventory-guard-present ×2**: two new codec files grew unset-pubkey
  guards; recorded so their loss trips the ratchet, per this class's own note.

Convergence addendum (CLOSEMAKER landing, 2026-08-31): MIGRATE's
`registry/declare_successor_v1/tests.rs` reached main without its two
`debt-derivation-restatement` rows — the seam gate was red on main itself, not
on any lane. Recorded here with the class's standing reason (a test file
spelling the record-contract's seed tuple, like its many sibling test files)
so the landed tree is green; the fix remains the class's shared-constructor
refactor.

---

## 2026-08-31 — cohort-9 FRACCHECK-7: one new tag, and the reason it is not `hazard-unset-pin`

### benign-typed-nonzero-wire

**1 entry.** `programs/dclutch-claims-sbf/src/fractional_claim_check_v1.rs` ·
`authenticate_fractional_compaction`.

The reader is right about the function and wrong about the route, and the
difference is the whole reason this is a separate tag. The function does pin the
System program by key, and it does authenticate a coordinate against a wire
pubkey — `root_account.key.to_bytes() != request.root()` — with no all-zero
refusal lexically inside it. What the reader cannot see is that the value it is
comparing against **cannot be zero, by type**.

`request.root()` reads `TerminalSettlementRequestV3`'s `owner`.
`TerminalSettlementRequestV3` is a tuple struct with a private field; its only
constructors are `new` and `decode`; `decode` routes through `new`; and `new`
runs `nonzero` over eighteen identities, `owner` among them. So there is no
value of that type carrying a zero owner for the route to read — not "the
derivation would catch it later", but "the value does not exist".

**Why not `hazard-unset-pin`.** That class's own note is the standard this had
to clear: its eighteen entries are recorded because *"fails somewhere
downstream is an argument, not a guard, and the downstream check is one
refactor away from moving."* This one does not fail downstream. It fails
**upstream**, at a constructor no caller can go around, and a refactor cannot
move it without making the private field public or adding a third constructor —
either of which is a visible change to the codec, not a quiet drift in a route.
Filing it as a hazard would say this tree carries nineteen unguarded frames when
it carries eighteen, and a register that overstates is worth as little as one
that excuses.

**It is not taken on argument.** Two witnesses landed with this entry, and the
second was written only after the first turned out vacuous:

- `terminal_settlement_v3::tests::no_identity_on_this_wire_may_be_the_unset_pubkey`
  — the guard's first witness ever. It sweeps every 32-byte window of a
  canonical encoding, zeroes it, and requires the decode to refuse by name;
  exactly eighteen windows do. Stated over all eighteen rather than over `owner`
  alone, because a test naming one field goes on passing while a later edit
  drops any of the other seventeen from the array, and the array is the only
  thing holding them. Mutation-proven: removing `input.owner` from that array
  reds it at seventeen.
- `fractional_claim_check_v1::frame_guard_tests::an_unset_owner_coordinate_is_refused_before_any_account_is_read`
  — the route half, with a note recording that the obvious version of it was
  vacuous. Asserting only that a zeroed-owner wire refuses `0x5642` at the route
  proves nothing: these synthetic frames carry empty account data, so a
  well-formed request refuses `0x5642` too, from a derivation further in. Same
  code, different cause. The discriminating assertion is therefore made against
  the decoder, and the route assertion is kept only to pin which code a
  validator log will show.

**inventory-guard-present ×3** land in the same commit and are not findings:
`claims-svm/fractional_claim_check_compaction_receipt_v1.rs` (this thread's own,
from FRACCHECK-6's receipt), `product-payoff-v2-codec/runtime_v3.rs` and
`product-runtime-v2-svm-reader/lib.rs` grew unset-pubkey guards. Three more
files that refuse the unset pubkey, recorded so losing the last guard in any of
them trips the ratchet — per that class's own note, this is three new guards and
not three new defects.

Baseline edited **by hand**, never `--write`: the FRACCHECK-2 precedent holds,
because `--write` retriages the whole register against a committed tree and
would have swept these four in under whatever tag the reader defaulted to,
which is exactly the adjudication this entry exists to make.

---

## 2026-08-31 — cohort-9 PROFILE-2: the succession rename, and two rows main was red without

### debt-derivation-restatement, +19 / −13

No new tag and no new argument. The population moved because the
infrastructure profile's PDA domain changed name, plus two rows this lane did
not author.

**Seventeen are the same debt under a new constant.** The profile succession
(`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md`) flipped every consumer
from `PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1` to its `_V2` successor.
The register keys a finding by `SYMBOL\tfile`, so each of those sites left as a
`_V1` row and arrived as a `_V2` row — the same second author for the same
address, in the same file, restating the same kind of seed tuple. Thirteen `_V1`
rows are gone for exactly that reason and not because anything was fixed; the
count differs from seventeen because four sites are new readers this lane added
and the successor bootstrap's own `_V1` rows stay, that tool still reading the
predecessor profile until it gains a ceremony stage. The class's standing
reason carries over unchanged, and so does the real fix: one shared derivation
helper exported by the crate that owns the domain.

**One is the ceremony route's predecessor read.** `infrastructure_v2.rs`
restates the `_V1` domain because conjunct 2 authenticates the predecessor
profile at its own address — the one place in the tree that must derive the
older domain on purpose. It reached main unrecorded when the route landed, so
the gate was red on main rather than on any lane, exactly the shape the
CLOSEMAKER addendum above describes.

**Two are FRACCHECK-7's**, `RAW_RECORD_PDA_SEED_V1` and
`STAGING_CURSOR_PDA_SEED_V1` in
`programs/dclutch-claims-sbf/program-test/fractional-atomic/src/campaign_support.rs`.
A campaign support file spelling the record contract's seed tuple, like its
many sibling test files. Recorded here with the class's standing reason, on the
CLOSEMAKER precedent, so the landed tree is green — flagged as not this lane's
authorship rather than absorbed silently.

Retriaged with `--write` and then verdicted by hand. `--write` was safe here
because it tags everything it does not recognise `untriaged` rather than
guessing, and the gate refuses an untriaged row — so nothing could enter under
a default tag without this entry being written. It also declined to read the
successor bootstrap's files while another author had them uncommitted, which is
why that tool's rows are untouched above.

### debt-derivation-restatement, +4 (PROFILE-2's own builder)

`crates/dclutch-operator/src/infrastructure_succession_v1.rs` restates four
domains: both infrastructure profile PDA domains, and the record contract's raw
and staging seeds.

All four are the same second-author debt the class has always described, and
they are unavoidable in this file for a reason worth stating. The builder
re-derives every address the succession ceremony will read, because its whole
contract is to refuse locally whatever the chain would refuse -- a builder that
took an address on trust would compose frames the chain rejects, which the
crate's own header calls not a service to the caller. It derives BOTH profile
domains because the ceremony spans them: the successor at its own address, and
the predecessor at the address it has always had.

The fix is the class's standing one, a shared derivation helper exported by the
crate that owns each domain, and it would retire these four with the rest.

Two rows left the register in the same measurement and were NOT fixed by this
lane: the fractional-atomic campaign's raw-record and staging-cursor
restatements, which their own author repaired. The population shrinking there
is a repair; the population growing here is not.

### debt-derivation-restatement, +4 (the succession campaign)

`programs/dclutch-core-sbf/tests/infrastructure_succession_program_test.rs`
restates the same four domains its builder does: both infrastructure profile
PDA domains, and the record contract's raw and staging seeds.

A real-ELF campaign has to plant the world it tests, and planting an account
means naming its address. The two profile domains appear because the ceremony
spans both -- the successor's vacancy and the predecessor's written account are
different assertions about different addresses. The record seeds appear because
the campaign plants finalized artifact records for the Registry and Rent
releases the succession selects.

The class's standing reason and standing fix apply unchanged. This file is one
of the many test files the note already describes.

---

## 2026-08-31 — cohort-9 CLOSE-DRIVER: four restatements fixed, one new tag, three attributed elsewhere

The maker-replay close landed its operator half — a plan builder in
`dclutch-operator` and the two subcommands that drive it — and the seam went red
on ten findings. Four were fixed outright, three belong to another lane's file,
and three are records about this one.

### The four that were fixed, not verdicted

`crates/dclutch-operator/src/direct_close_maker_v1.rs` and
`tools/local-validator/bootstrap/successor/src/direct_close_maker.rs` each
restated the raw-record and staging-cursor seed tuples. Both now take the seeds
from their owner through `RecordKeyV1::raw_record_pda_seeds` /
`staging_cursor_pda_seeds` and a local `record_address` that places
`seeds.domain()` rather than naming it — the pattern `964549dd` set in the
retiring test that morning.

The builder's two bump-bearing derivations went the same way, through a
`record_address_at_bump` over the same seed material, even though the reader had
not flagged them: leaving the tuple spelled in two of a file's four derivations
would have retired the finding without retiring the defect.

### `checked-caller-excludes-payer`, 2 entries

`assemble_plan` in `direct_close_maker_v1.rs` censuses `is_signer` across its
whole 22-account frame and pins exact writability on every coordinate, so it
draws both halves of class 6 — exactly as its sibling
`direct_begin_retiring_v1.rs assemble_plan` does, under `hazard-signer-census`
and `hazard-privilege-pin`.

This tag exists because the standing question those two tags carry — *does any
builder of this route place a signing account in this frame?* — has now been
asked and answered for this route, and the answer changed the code.

**The answer is yes, and it is the obvious way to run the close.** The route's
on-chain frame check (`direct_close_maker_v1.rs:134` in the Trading program)
refuses any signer and pins each coordinate's writability, and both are
transaction-level: a fee payer signs and is written for the fee whatever
`AccountMeta` it carried. The frame's coordinate 21 is the maker's recorded
`rent_owner`. So a maker closing their own replay and receiving their own rent —
the first thing anyone would try — would have refused on chain as
`CloseMakerFrame`, with nothing in the message to say why.

The sole builder of this route now refuses that before it sends:
`refuse_payer_in_frame` in the subcommand names the colliding coordinate and
says to pay from a stranger, held by
`a_fee_payer_the_frame_already_names_is_refused_before_the_send`.

That is this class's harm statement in negated form, which is what retired the
three `market.rs` censuses on 2026-08-30. Those could leave the register because
their exclusion sits in the same function the reader was reading. This one
cannot: the census is in `dclutch-operator` and the exclusion is in the
successor bootstrap, two crates apart, and no static reader that pairs them by
proximity will ever see it. Hence a tag rather than a departure — the finding is
real, the question is closed, and the closure is not visible from where the
finding is raised.

Not `hazard-*`, because those mean nobody has looked yet, and recording it that
way would send the next reader to redo work that is already done and tested.

### `inventory-guard-present`, +1

`direct_close_maker_v1.rs` refuses the unset pubkey in its coordinate closure,
so it joins the ratchet. One new guard, not one new defect, per that class's own
note.

### `debt-derivation-restatement`, +3 (PROFILE-3's succession caller)

`tools/local-validator/bootstrap/successor/src/infrastructure_succession.rs`
restates the infrastructure-profile domain's 1-seed tuple and the record
contract's raw and staging seeds. The file arrived in `2a10fa4c`, PROFILE-3's
cut-day caller for the succession ceremony, and its findings are its frame
rather than this lane's; they surfaced in the same measurement window only
because both lanes landed against one baseline.

Verdicted with that attribution rather than repaired, because the file has an
owner who is still working in it. The class's standing fix applies unchanged and
is now cheap: the accessor pattern this lane just applied twice, four files down
the same seam, will retire all three.

Baseline edited **by hand**, never `--write`: the FRACCHECK-2 precedent holds,
and `measured_commit` is left where `--write` last set it.

---

## 2026-09-01 — cohort-9 SEAM-VERDICT: the completion wave's 46, and one guard that was never held

The overnight completion wave took the gate red on **46** findings at
`10f8e3e2` — not the eleven a first reading of the report suggested, because
fifteen of them are `UNSET_GUARD_PRESENT` inventory rows and thirty are
findings in four other classes. One fix landed, four tags are new, and every
row below was read before it was written.

### benign-decoder-refuses-unset

**1 entry.** `programs/dclutch-trading-sbf/src/dealer/v4_lp_accelerator_accounts.rs` ·
`authenticate_position` — and the test that was missing under it.

**The question.** The function derives

```rust
let expected = Pubkey::find_program_address(
    &[
        DEALER_LP_POSITION_PDA_DOMAIN_V3,
        &request.child_root,
        &request.lp_owner,
    ],
    &trading,
).0;
```

and pins `system.key == &system_program::ID`. `system_program::ID` **is** the
all-zero pubkey — verified, `declare_id!("11111111111111111111111111111111")`
decodes to 32 zero bytes — so the reader is looking at a function where the
unset pubkey is legitimately meaningful, and it fires. The question it cannot
answer is the one that matters: can an attacker put an all-zero `lp_owner` or
`child_root` into that derivation?

**The answer is no, and it is refused two independent ways.**

*At the decoder.* `DealerMultiLpRequestV3::decode` reads `child_root` at offset
80 and `lp_owner` at offset 144 through `read_identity`, which is
`read_identity_or_zero` plus `if value == [0; 32] { Err(InvalidRequest) }`.
Seven of the wire's eight identities go through it; only `lp_digest` at 240
uses the permissive form, and Open *requires* that one unset. `decode` is the
sole ingress: `evaluate_authenticated_dealer_lp_v4` builds its `request` at
line 78 from `DealerMultiLpRequestV3::decode(invocation.family_request())` and
nowhere else, and the off-chain builder
`dclutch-operator/src/dealer_lp_hot_v4.rs:133` goes through the same call.

*At the context join.* `authenticate_context` runs before
`authenticate_position` and requires `request.child_root ==
context.root.to_bytes()`, then `root.key.to_bytes() == request.child_root` with
`root.owner == trading_program`. The child root is pinned to the authenticated
invocation, not chosen at the wire.

The collision half of the question is a non-question: a distinct `lp_owner`
gives a distinct PDA, so a zero owner could only ever derive *its own* address,
never substitute for a live one — and `position.key.to_bytes() ==
request.lp_position` plus `LP_OWNER_IDENTITY_V3 == request.lp_owner` in the
transition bank bind it three more times.

**Why not `benign-typed-nonzero-wire`.** That tag's bar is *"the value cannot
be zero, by type"* — its subject is a tuple struct with a **private** field
whose only constructors run `nonzero`. `DealerMultiLpRequestV3` has **public**
fields, so a struct literal is writable in principle and the guarantee is not
type-enforced. It rests instead on `decode` being the only constructor, which
is a **census** fact: the two `DealerMultiLpRequestV3 {` matches in the tree are
the definition and the `impl`, there is no `Default` derive, and there is no
struct-literal site anywhere. That is one honest notch weaker than the
precedent, and it gets its own tag rather than borrowing that one's strength.
Making it type-enforced would mean privatizing the fields — a visible codec
change, and the right fix if this route ever grows a second constructor.

**Not taken on argument.** The precedent's standard is a named, mutation-proven
witness, and this route had none: `open_is_chain_derived_and_hostile_bytes_refuse`
mutates the magic/version/reserved bytes, zeroes the rent, and substitutes
`lp_position`/`obligation_digest` — it never zeroes an identity. So this lane
wrote the witness rather than verdicting without one:

`v3_operator::tests::no_identity_on_this_wire_may_be_the_unset_pubkey` sweeps
every 32-byte identity window of a canonical Open encoding, zeroes it, and
asserts the set of offsets that refuse is exactly `[16, 48, 80, 112, 144, 176,
208]` — stated over all seven rather than over `lp_owner` alone, because a test
naming one field goes on passing while a later edit moves any of the other six
to `read_identity_or_zero`. It also pins that offset 240 is already zero, and
that `build_open_lp_v3` refuses a zero owner at the other end of the wire.

Mutation-proven: changing `lp_owner: read_identity(bytes, 144)` to
`read_identity_or_zero` reds it at six offsets instead of seven. The change is
test-only — 92 insertions, no production line touched.

### inventory-nonzero-guard-not-pubkey

**5 entries. Not findings.** `UNSET_GUARD_PRESENT` says *"this file refuses the unset pubkey somewhere in
it"*. For eleven of the wave's fifteen that is true and they are filed
`inventory-guard-present`. For these four it is **false**, and filing them as
that tag would put a false sentence in the register:

- `crates/dclutch-product/src/payoff/spline_eval_v3.rs` — the match is
  `pub fn is_zero_at(&self, claim: usize) -> bool` over a `U256` B-spline weight
  numerator. It returns a `bool`, refuses nothing, and **`grep -c Pubkey` on the
  file is 0**: it is fixed-point spline arithmetic containing no address type.
- `programs/dclutch-trading-sbf/src/series/retire_funding_artifacts_v5.rs` — the
  match is `InstructionV3::nonzero(s(SERIES_RETIRE_OBSERVED_OUTSTANDING_SCALAR_V5))`,
  a nonzero constraint emitted over a **u64 scalar register** in interpreter
  bytecode. `grep -c Pubkey` is also 0 here.
- `crates/dclutch-operator/src/series_current_acquisition_v5.rs` and
  `programs/dclutch-trading-sbf/src/dealer/v4_lp_accelerator_accounts.rs` — both
  genuinely refuse an all-zero value, but the operands are **content digests**
  (`artifact_release`, `checked_manifest_digest`, `LP_PRESTATE_DIGEST_IDENTITY_V3`),
  not addresses. Neither file contains `Pubkey::default()` at all.

The tag records what these actually ratchet: a nonzero guard over something
that is not a pubkey. The ratchet still turns — losing these guards still fails
the gate — so nothing is weakened. What it stops is the register asserting an
address check that is not there, and it leaves the next reader the exact
statement of the matcher gap: the class's zero-comparison spellings
(`is_zero()`, `nonzero`) are not pubkey-typed. **Tightening the matcher is not
this lane's change** — it would re-measure all 271 existing inventory rows and
mass-`GONE` the register, which is a lane of its own.

### benign-sole-seed-no-helper

**9 entries.** `PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1` ×3 and `_V2` ×6, across
`successor/src/campaign.rs`, `market.rs`, `direct_market.rs`,
`series_premarket_expiry_chain_v1.rs`, `plan.rs`, `runtime.rs` and
`crates/dclutch-release-tool/src/infrastructure.rs`. The last three arrived
untriaged and were verdicted here on 2026-09-03; each imports the domain from
its owner and spells `&[DOMAIN]`, arity one, exactly as the six before them.

The reader reports these restate "the 1-seed tuple for a domain owned by
`crates/dclutch-registry`, which exports `CallerAuthoritySeedsV1`
for exactly this". **The second half is wrong.** `CallerAuthoritySeedsV1` is a
*six*-seed projection over a *different* domain,
`CALLER_AUTHORITY_PDA_DOMAIN_V1`. For the infrastructure-profile domains the
owning crate exports the constant and its length assert and **no seed helper at
all**.

So there is no "one domain spelled two ways" here and no standing fix to apply:
each site imports the domain constant from its owner and the only restated fact
is *arity one*. All 56 such tuples in the tree are arity 1. Filed apart from
`debt-derivation-restatement` because that tag promises an adoptable helper,
and pointing a future reader at a helper that does not exist wastes exactly the
work the register is supposed to save. If the owner ever grows one, these six
become ordinary debt.

### benign-fixed-width-array-seed

**1 entry.** `crates/dclutch-source` · `PYTH_RELEASE_RECORD_SCHEMA_ID_V1`.

Reported as "32 bytes, within the maximum today, but no `const _: () =
assert!(… .len() <= 32)` holds it there". The constant is
`pub const PYTH_RELEASE_RECORD_SCHEMA_ID_V1: [u8; 32]` — a **fixed-width
array**, not a `&[u8]`. Its `.len()` is a property of the type, so the proposed
assert is trivially true and cannot be falsified by any edit preserving the
type; and an edit that changes the type breaks the `[u8; 32]`-typed consumers
(`staging`, `authenticate_raw`, `record_pair`, `SchemaReleaseId::new`) at
compile time, not at derivation time. All 22 such asserts in the tree guard
`&[u8]` domain constants and none guards a `[u8; N]`: the idiom deliberately
does not reach this shape. Adding the assert to silence the reader would put a
tautology in the source, which is the decoration this register exists to refuse.

**One real thing was found under it, and it is not this class.** Every
neighbouring release id (`RESOLUTION_CONTROLLER_RELEASE_ID_V3…V7`) pairs with a
`..._PREIMAGE_...` constant and a doc-comment reading *"SHA-256 of […]"*.
`PYTH_RELEASE_RECORD_SCHEMA_ID_V1` has no preimage, no derivation doc, and no
test that recomputes it — an opaque literal in a row of reproducible ones, used
as seed #2 of the record tuple. Not filed as a seam finding because it is not
one; recorded here so it is not lost.

### `debt-derivation-restatement`, +15

Two `LIABILITY_BASIS_MARKET_SEED_V2`, one `LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2`,
seven `RAW_RECORD_PDA_SEED_V1`, five `STAGING_CURSOR_PDA_SEED_V1`.

All fifteen were checked seed-for-seed against the owning crate's helper —
`LiabilityBasisMarketSeedsV2`, `LifecycleRentCreditPdaSeedsV2`,
`RecordPdaSeedsV1` — and **all fifteen agree**: same domain bytes (every site
imports the constant, none re-spells the literal), same arity, same order, same
operand meaning. Ordinary debt of the known shape, not a live derivation
defect. Two notes for whoever retires them:

- The reader names the wrong helper for `LIABILITY_BASIS_MARKET_SEED_V2` — it
  lists the `ClaimCheck*` exports; the real one is `LiabilityBasisMarketSeedsV2`
  in `claims-svm/src/liability_basis_state_v2.rs`, whose own doc-comment names
  this defect class and which is adopted at only 2 of ~23 sites.
- `RecordPdaSeedsV1` has no `as_slices()`; callers must still author the ordered
  array. "Call the helper" here means `RecordKeyV1` plus a local assembler,
  which is weaker containment than the claims and rent helpers give.

Not repaired in this lane: several of these files have owners still working in
them, and the register's job here is to say the debt is known, agreed, and
shaped, rather than to reach into four other lanes' files on a gate run.

### benign-declared-privilege-census

**2 entries.** `crates/dclutch-general-successor-operator/src/lib.rs` · `parse_route_v1`
and `programs/dclutch-trading-sbf/src/core_composition_v3.rs` · `prepare`.

Was three: `parse_route_v1` used to draw **both** class-6 codes, and its
`PRIVILEGE_PIN_UNEXEMPTED` half left on 2026-09-03 when the matcher learned to
read `meta.is_writable != (index == HOT_ROOT_ACCOUNT_V3)` as the exemption it
is. The signer half stands, on the argument below.

Both hazard notes rest on one premise: `is_signer` and `is_writable` are
**transaction-level** properties of a runtime account, so a frame-wide refusal
is dead for any builder that pays with an account the frame names. These three
entries pin bits that are **not** that.

`parse_route_v1` is a hostile decoder for an **operator-authored JSON
document** — `pub fn parse_route_v1(bytes: &[u8])`, bounded at
`MAX_ROUTE_BYTES_V1`, reached through `read_bounded_route_file_v1(--route
<file>)`. The `is_signer` it refuses is a `"isSigner": false` field in that
document, not a privilege the runtime assigned. Refusing it yields a parse
error before any instruction exists. The fee payer is a **separate top-level
field** (`route.payer`), never a member of either pinned vector, and the
payer's own logical coordinate is declared `exact_rule(signer: true, writable:
true, …)` in `general-adapter-contract/src/account_rules_v3.rs` — it lives in
`runtime_suffix_accounts`, which this function does not pin at all.

`prepare` does iterate a whole frame — `frame.iter().any(|account|
account.is_signer)`, twice, over widths 25 and 26. But `frame` is not the
incoming account array: it is filled by `gather_invocation_accounts` from a
`DowngradedEffectAccountsV3`, whose `view()` **overwrites both privilege bits
from the artifact's declared AccountProfile byte** before the account is ever
seen here:

```rust
logical.is_signer = declared & DECLARED_SIGNER_V3 != 0;
logical.is_writable = declared & DECLARED_WRITABLE_V3 != 0;
```

(`hot_v3.rs:6952-6953`, read directly.) `require_child_route_privileges_v3`
confirms the separation from the other side: it compares `writable` and
`executable` against the physical account and deliberately never compares
`declared.signer()` to `account.is_signer`. So a fee payer reading `is_signer
== true` at the transaction level does not set the declared bit and does not
trip this census. What the check actually asserts is that the artifact's
profile declares no signer for a permissionless verb.

Not `hazard-*`, because those mean nobody has looked; the looking here found
the class's premise absent rather than unexamined.

**One thing to route onward, not filed here.** Core's parser for the same
permit-expiry route (`core-sbf/src/series_permit_expiry.rs:124-134`) documents
an exemption *by name* for its 26th coordinate — the funded-crank recipient,
"usually the fee payer, who signs" — and calls refusing it "the live defect
that keeps a cleanup's beneficiary from paying its own fee", citing
`docs/design/FUNDED_CRANK_V1.md` §6. Trading's census and Core's exemption do
not currently collide: Trading's 25-wide window excludes the crank, and the
26-wide precommit slot holds a caller-authority PDA that
`authenticate_precommit_caller_v1` separately pins non-signer. But that is the
one place this class would bite if a crank recipient ever reached Trading's
frame. It belongs to whoever owns FUNDED_CRANK_V1 §6, not to a gate run.

### benign-payer-pinned-signer

**1 entry.** `programs/dclutch-claims-sbf/src/fractional_claim_check_v1.rs` ·
`process_fractional_redemption`.

A real on-chain exact-privilege pin over all nine coordinates, both bits, from a
table. It draws the class correctly and the harm still cannot occur, because the
table **admits** the payer rather than excluding it:

```rust
Self::Holder => (true, true),
```

Coordinate 0 is pinned `is_signer == true, is_writable == true`, so a holder
redeeming their own claim and paying their own fee satisfies the pin exactly —
the transaction-level bits the class warns about are the bits this frame
requires. The remaining eight are a mint, the Token-2022 program, two PDAs and
three token accounts, none of which can be a fee payer. Builder and program
read the same `role.privileges()` table, so the two sides cannot drift apart.

Distinct from `checked-caller-excludes-payer`, and stronger: that tag records an
exclusion made in a builder the reader cannot see, one crate away. This needs no
exclusion and no cross-crate argument — the admission is in the pinned table
itself, where any reader of the frame will find it.

### benign-named-coordinates-only

**1 entry.** `crates/dclutch-operator/src/series_hot_v3.rs` · `build_selected_series_hot_v5`.

**The finding's premise is absent.** The class reports a refusal of "every
signer across the whole frame". Over the function's whole body there are exactly
three `is_signer` refusals and each names a single coordinate: the Trading
program account and the capability root PDA, plus the shadow caller authority
PDA on the Consume action. None of the three can be a fee payer. The only
whole-frame iterations in the function are an observation/finality sweep and two
membership searches; neither reads a privilege bit.

The payer is not merely unexcluded here, it is **required present and left
uncensused** — `roles.payer` is one of six keys the function requires to appear
among the built accounts, with no signer refusal applied to it.

The reader most likely paired a `.iter()` elsewhere in the function with the
single-coordinate refusals. A genuine per-coordinate census does exist in this
file, at `validate_runtime_profile`, but it is a different (V3) function and an
exact profile-driven pin (`account.is_signer != (privileges & 1 != 0)`) rather
than a blanket refusal.

Recorded rather than departed because the register's ratchet is keyed on the
finding set: silently dropping it would fail the gate as `GONE` on the next run.

### The two that left: one rename, one real fix

The ratchet turned the other way twice, and the two resolve **differently** —
which is the whole reason `GONE` is not auto-cleared.

**`execution_strategy_v2.rs · authenticate_common_frame_with_sealed_capability_alias`
was renamed, not fixed.** Commit `1e5c8343` "trading: preserve authenticated
sealed record pairs" renames it to `..._pair` in place, widening the signature
with `capability_program_id: ContentId`, and renames its sole call site in the
same hunk. The pinned block is unchanged context in that diff, and the pin was
*strengthened* afterwards with a new `staging` coordinate. `_alias` exists
nowhere at HEAD. So the `NEW ..._pair` row and the `GONE ..._alias` row are one
finding wearing two keys: the entry is re-keyed and **carries its
`hazard-privilege-pin` tag forward**, rather than being triaged as fresh.

It is left as a hazard deliberately. A first reading says it is benign — all
four pinned coordinates are a program, a sysvar, and two off-curve PDAs, none of
which can sign a transaction — but `hazard-privilege-pin` means *nobody has
enumerated the route's builders*, and this lane did not do that enumeration. A
gate run is not the place to spend a hazard's standing question on a plausible
argument. The lead is recorded; the question stays open.

**`hot_v3.rs · authenticate_lifecycle_credit_v3` was genuinely fixed**, and its
entry leaves the register. At the baseline the function ended with a frame walk
whose predicate is exactly what the class matches:

```rust
|| !accounts.iter().any(|candidate| {
    candidate.key == account.owner
        && candidate.executable
        && !candidate.is_signer
        && !candidate.is_writable
})
```

Commit `686bf2e5` "trading: bind lifecycle credit to fixed registry" deletes the
walk outright and takes an already-authenticated `owner_program: &AccountInfo`
as a parameter, pinning two named coordinates instead of searching the frame.
The function and file both still exist and the key is unchanged — it is the
pattern that stopped matching, which is the good reason for it to stop.

### Scope, honestly

The lane was briefed on eleven findings and the tree had **46**. The other
thirty-five were measured at the same commit and are the same completion wave's;
the gate is all-or-nothing, so taking CI green meant reading all of them. Four
tags are new because four situations the register had no vocabulary for arrived
at once, and the alternative — filing them under the nearest existing tag —
would have put four false sentences in a register whose entire purpose is that
its sentences are true.

**Two matcher gaps are now named and neither is repaired here.** The
`UNSET_PIN` inventory matcher is not pubkey-typed (see
`inventory-nonzero-guard-not-pubkey`), and class 6 pairs frame iteration with
single-coordinate refusals (see `benign-named-coordinates-only`). Both fixes
re-measure hundreds of existing rows and mass-`GONE` the register; each is a
lane of its own, and doing either inside a gate-greening run would have hidden
the greening under a rewrite.

Baseline edited **by hand**, never `--write`: the FRACCHECK-2 precedent holds,
and `measured_commit` is left where `--write` last set it.

## 2026-09-01 — `provider_transport_v3.rs`, one new `inventory-guard-present`

**Not a finding. A capability arrived and brought a guard with it.**

`programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs` newly
refuses the unset pubkey at `:398`, because `AbandonSubmission` (`DCLTPAB3`)
landed in that file — the route that lets a provider submission which **lost the
first-valid race** reclaim its rent instead of stranding 6,389,280 lamports
forever.

That route's admissibility rests entirely on statements about zero: it checks
the lifecycle's `terminal_sequence`, `certificate` and `provider_evidence` each
as zero **rather than trusting `status`**, precisely because a `Submitted`
lifecycle carries zero in all three by construction and that is what makes the
consumed wire unable to express an abandoned submission. A guard refusing the
unset pubkey is exactly the shape that argument needs.

Filed `inventory-guard-present`, which is the true sentence here: this file does
refuse the unset pubkey somewhere in it. The entry exists so **the gate fires if
the last such guard in this file is ever deleted** — which, for a route whose
whole safety case is "these identities are provably zero", is the entry most
worth having.

## 2026-09-01 — two `validate_frame` entries removed because the function is gone

`programs/dclutch-resolution-proof-sbf/src/lib.rs` no longer contains
`validate_frame`. It lived inside the 775 lines of `#[cfg(any())]` dead code
deleted in `2eebff33`'s neighbourhood — a superseded V1 path kept beside its
successor, opening with a 248-line block named
`removed_legacy_v1_direct_instruction` that had never been removed.

So both baseline rows — `hazard-privilege-pin` and `hazard-signer-census` — were
reported **GONE** by the gate, which is the correct verdict and the correct
instruction: *if it was fixed, the register should shrink.*

**Removed by hand rather than by `--write`.** The standing rule against
`--write` exists because that flag would also silently absorb any **NEW**
finding in the same pass, converting a seam disagreement into a baseline entry
nobody argued for. Deleting the two stale rows by line, verifying the file still
parses, and confirming the only surviving `validate_frame` row belongs to a
different file (`core-sbf/src/begin_retiring.rs`, which does still contain one)
gets the same shrink with none of that risk.

Worth noting what the pair actually were, since the register no longer will:
both were findings **about code that never compiled.** A `cfg(any())` block
cannot execute, so a privilege pin or signer census inside one describes a
hazard no transaction could ever reach. They were true statements about text.

### benign-cache-authenticated-by-admission-token

`AUTHORITY_CACHE_UNDERIVED`, two sites, both `programs/dclutch-custody-sbf/src/lib.rs`:
`authenticate_realm` and `authenticate_premarket_realm`.

Both decode the Registry activation cache and read a role out of it without
deriving the cache address or checking its owner in their own bodies. The
provenance is established one call earlier and carried in the TYPE, which a
single-function reader cannot see: `authenticate_market_admission` resolves the
cache exactly once and returns `AuthenticatedMarketAdmissionV1`, whose two
variants are the two authenticated outcomes —
`Live(authenticate_market(..))`, which delegates to
`authenticate_activation_cache_bump_v1`, and `Premarket { cache_bump }` from
`try_authenticate_premarket_market`. Both arms carry a `cache_bump`, which is
the derived-address token. The realm functions are reachable only by matching on
that value (`lib.rs:301-308`), so the authentication is a precondition of
constructing the scrutinee rather than a convention.

Verified by reading both arms, not inferred from one. This is a *good* pattern —
authenticate once, prove it in the type — and the finding is an artefact of the
reader's one-function horizon, recorded rather than suppressed so that the day
someone calls these functions from a third path with no token, the tag is a
claim that has to be re-checked.

**What this tag does not assert:** that the role read is the *right* role for
the act. Provenance and correctness are different questions; this reader answers
the first.

### hazard-cache-provenance-unverified

`AUTHORITY_CACHE_UNDERIVED`, one site:
`programs/dclutch-trading-sbf/src/hot_v3.rs` `selected_role_programs_v3`.

Reads `frame.activation_cache`, decodes the view and selects Claims, Custody and
Resolution role programs from it, with no derivation, no owner check and no
delegation in its own body. The same admission-token argument that clears the
two Custody sites is *plausible* here — `HotFrameV3` is built upstream and the
route is long — **but this lane did not establish it**, and a tag is a claim.

Recorded as a hazard rather than as benign for exactly that reason: the honest
difference between the Custody sites and this one is that the Custody chain was
read end to end and this one was not. `AGENTS.md`'s rule about absent signals
applies to verdicts too — "not shown to be wrong" is not "shown to be right".

Owner: the Trading lane. Closing it means one of two things, both cheap:
show that every path reaching `selected_role_programs_v3` authenticates
`frame.activation_cache` first and retag it, or route the read through
`authenticate_activated_role*` so the question stops needing an argument.

**CLOSED `d211cd72`, hours after it was filed**, by the second of the two — the
read now authenticates itself rather than assuming an upstream did. The baseline
entry is deleted rather than retagged benign, because the rule for this register
is the one at the top of this file: keep an entry only while it is true. The tag
is kept here with no users, as the record of a hazard that was named and then
answered; the next site to earn it will find the reasoning already written.

Worth keeping for the method rather than the outcome: the honest verdict at
filing time was *hazard*, not *benign-same-pattern*, purely because the Custody
chain had been read end to end and this one had not. Had it been waved through
on the resemblance, the fix would not have been written.

---

## 2026-09-03 — SEAM: 45 gate failures, six repaired at the author, three reader gaps, twelve verdicts

The gate stood at **45 failures against `e6b7bf1af`: 27 GONE, 17 NEW, 1
UNREASONED**, four of the NEW already sitting `untriaged` in the register. 629
commits had landed under the baseline. Every one of the 45 is dispositioned
below and each disposition cites its site.

Counts, before and after: **17 NEW → 0**, **27 GONE → 0** (the register shrank
by them), **1 UNREASONED → 0**, register **706 → 683 entries**, `untriaged`
**4 → 0**, `confirmed-defect` **4 → 1**, `AUTHORITY_CACHE_UNDERIVED` **2 → 0**.

### Nine NEW were repaired, not verdicted (`d8a679168`)

- **`AUTHORITY_CACHE_UNDERIVED` ×2** — `programs/dclutch-custody-sbf/src/lib.rs`
  `authenticate_calling_release_from_cache:1055` and
  `authenticate_realm_from_cache:1078`. Both decoded the Registry activation
  cache and read a role out of an account neither body derived or owner-checked.
  The provenance did hold: all five call sites in
  `dealer_reservation_v1.rs` (`:351`, `:656`, `:871`, `:1217`, `:1373`) call
  `authenticate_market_from_cache` first, on the same frame vector, and that one
  does `require_cache_account` + `authenticate_activation_cache_identity_v1`.
  But it held **by call order**, which no type and no reader enforces — and the
  entry that used to cover this shape,
  `benign-cache-authenticated-by-admission-token`, rested on a `cache_bump`
  carried in `AuthenticatedMarketAdmissionV1`, and that field **no longer
  exists**: `9b5de611e`/`5709672aa` reduced the variant to a bare `Premarket`.
  Its own closing sentence said this day would come — *the day someone calls
  these functions from a third path with no token, the tag is a claim that has
  to be re-checked*.
  Repaired by collapsing the three `*_from_cache` wrappers into one
  `authenticate_reservation_frame_v1`: one borrow, one decode, one identity, one
  view handed to market, calling release and realm. Each reservation route goes
  from THREE full five-role decodes to one.
- **`AUTHORITY_CACHE_UNDERIVED` ×1** — `authenticate_market:787`. A reader
  defect, see below.
- **`SEED_DOMAIN_UNASSERTED` ×2** —
  `crates/dclutch-registry/src/release_set/generated_protocol_infrastructure.rs:44`
  and `:48`. `a00fc7c9d` moved both infrastructure profile PDA domains into the
  Lean emission and deleted their `const _: () = assert!(..)`, on the argument
  that `pda_domains_are_admissible_single_seeds` carries the bound. It does —
  over the Lean definition. The **checked-in Rust** was then guarded only by
  `check-generated.sh`, which needs `lake` and lives in the `emission` tier that
  records a missing prerequisite when a host has none. Repaired Lean-first: the
  emitter emits the assert beside each domain, so one author states the bound
  and `cargo check` also holds it. Regenerated through the emitter, `rustfmt`'d,
  `cmp`-clean.
- **`DOMAIN_BYTES_COLLIDE` ×1** — `dclutch:lbv2:market` under two names, the
  owner's `LIABILITY_BASIS_MARKET_SEED_V2`
  (`crates/dclutch-claims/src/generated_liability_basis_state_v2.rs:17`) and
  a fixture-local `CLAIMS_MARKET_SEED_V2` in
  `program-test/affine-batch/src/fixture.rs:42`,
  `program-test/fractional-atomic/src/narrow_fixture.rs:59` and
  `tools/fractional-exterior/src/narrow_fixture.rs:66`. All three crates already
  depend on the owner; they take the constant from it now, and
  `program-test/affine-batch/src/lib.rs:51`, which restated the owner's own
  name, does the same. Three `SEED_DOMAIN_UNASSERTED` rows leave with them —
  the fixture copies were the things being asserted about.
- **`PRIVILEGE_PIN_UNEXEMPTED` ×3** and **`TRANSACTION_LEVEL_SIGNER_CENSUS`
  ×0** — reader gaps, below.

### Three reader gaps, each fixed by name

**`AUTHORITY` read prose as code.** Custody's `authenticate_market` takes an
already-authenticated `ActivatedExecutionReleaseSetViewV1` as a **parameter**
and decodes no cache at all; it was reported because the comment above its
*market* decode explains what `ActivatedExecutionReleaseSetViewV1::decode`
costs. Every test in the class now runs over `_code_only`, which reuses the
`_code_mask` the survey already had. The noisy direction is the one that was
measured; the **silent** one is why it matters — a body whose doc comment writes
`authenticate_activation_cache_identity_v1(` or `.owner ==` read as delegating
or owner-checking while doing neither, and that finding would never have been
made.

**`PRIVILEGE_PIN_UNEXEMPTED` called five sites unexempted for a spelling.** The
matcher admitted `if index != COORDINATE {` and refused
`is_writable != (index == COORDINATE)` — the same statement with the coordinate
on the other side of the pin. One writable coordinate, named, every other
readonly, is the class's own definition of the fix (*an exemption, one
coordinate wide and reasoned in place*). Five sites carry it and one line each
says which coordinate:

- `crates/dclutch-operator/src/direct_begin_retiring_v1.rs:965` `!= (index == 0)`
- `crates/dclutch-general-successor-operator/src/lib.rs:477` `!= (index == HOT_ROOT_ACCOUNT_V3)`
- `programs/dclutch-resolution-proof-sbf/src/provider_instruction_v3.rs:462` `!= (matches!(index, 2 | 3) || index == tail_start - 1)`
- `programs/dclutch-general-accelerator-sbf/src/lib.rs:751` `!= (writable == Some(index))`
- `crates/dclutch-operator/src/dealer_equity_hot_v3.rs:328` `!= (pages == 1 && index == page_index)`

A second arm generalises `child_index != \d` from a literal digit to a **named**
coordinate, which retires the third NEW row,
`crates/dclutch-bearer-v2-operator/src/hot_transaction_v3.rs:292`
(`child_index != caller &&`). Two baselined `hazard-privilege-pin` rows and one
`benign-declared-privilege-census` row leave with them; each was read.

**Deliberately NOT admitted:** `observed.is_writable != expected.writable()`,
where the required writability is read per coordinate out of a declared frame
spec. Six on-chain sites carry it (`affine_batch_v2.rs:470`,
`protocol_position_v2.rs:828`, `sparse_native_transfer_v1.rs:455`,
`custody-sbf/lib.rs:1792`, `direct_replay_setup_v1.rs:379`,
`user_position_admission_v1.rs:201`) and it is a *different* argument — the
artifact declares the privilege, which is the `benign-declared-privilege-census`
family. Retiring those six is a reading of six frames, not a spelling fix, and
it is not this run's change. Named here so the next lane finds the list already
made.

**`DERIVATION` was blinded by a local wrapper.** `derive_hinted` is a
three-line `create_program_address`-at-recorded-bump helper that Claims and the
Dealer accelerator each grew while removing PDA searches, and moving the tuple
inside it took two restatement rows out of the register on 2026-09-02 **as
though someone had repaired them** — while both files still spell
`[LIABILITY_BASIS_MARKET_SEED_V2, &request.market]` in full
(`claims-sbf/src/signed_delta_v3.rs:826`,
`trading-sbf/src/dealer/v4_equity_accelerator_accounts.rs:467`). A `GONE` that
means *the reader stopped looking* is the one kind this ratchet must never
produce. `derive_hinted` and `derive_hinted_v3` are derivation patterns now, and
both rows come back — measured: exactly two, no collateral.

### Twenty-seven GONE, and why each left

- **10 `UNSET_GUARD_PRESENT` + 4 `DOMAIN_RAW_RESTATEMENT`** — whole files
  deleted. `53d73d4ee` deleted the `dclutch-fractional-claim-operator` V1
  generation (`artifacts.rs`, `claims.rs`, `records.rs`, `token2022.rs`,
  `tests/claims.rs`, `tests/support/mod.rs`); `4d13fe2af` deleted
  `trading-sbf/src/direct/` (`buy_escrow`, `complementary`, `inline`,
  `lifecycle`, `sell_escrow`).
- **1 `DOMAIN_RAW_RESTATEMENT`** `LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2` ·
  `trading-sbf/src/series/terminal.rs` — `cbdecdb3e` deleted the projected Hot
  V4 chain nothing dispatched.
- **2 `DOMAIN_RAW_RESTATEMENT`** `signed_delta_v3.rs` and
  `v4_equity_accelerator_accounts.rs` — these did **not** leave for a good
  reason and are back, see `derive_hinted` above.
- **1 `DOMAIN_BYTES_COLLIDE` + 2 `DOMAIN_NAME_BYTES_DISAGREE` + 2
  `SEED_DOMAIN_UNASSERTED`**, all `CLAIMS_FOUNDING_AGGREGATE_SEED_V4`/`_V5` —
  **the `confirmed-defect` was answered.** `b209be565` gave `LiabilityBasisV2` a
  Lean owner and rewrote both constants as *aliases*:
  `pub const CLAIMS_FOUNDING_AGGREGATE_SEED_V4: &[u8] =
  crate::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;`. The
  register's question was *"V4 beside V5 are byte-identical, so the version bump
  lives in the name and not in the address — someone who owns Claims founding
  has to say which it is"*, and an alias is that answer said out loud: one
  author for the bytes, and a reviewer following either name lands on the owner
  in one hop. `PROTOCOL_POSITION_STATE_SEED_V2` became an alias of
  `LIABILITY_BASIS_POSITION_SEED_V2` in the same commit.
- **3 `SEED_DOMAIN_UNASSERTED`** on the fixture-local `CLAIMS_MARKET_SEED_V2` /
  `LIABILITY_BASIS_MARKET_SEED_V2` — this lane's own fix, above.
- **2 `AUTHORITY_CACHE_UNDERIVED`** `authenticate_realm` /
  `authenticate_premarket_realm` — both now take the view as a parameter and
  decode nothing. This is a **rename with the reason NOT carried**: their
  successors are the `*_from_cache` wrappers, whose argument had changed, which
  is why they were repaired rather than re-tagged.
- **3 `PRIVILEGE_PIN_UNEXEMPTED`** — the spelling fix, above.

### Twelve verdicts, and the tag that is new

`debt-derivation-restatement` **×4**, all checked against the
2026-08-31 bar — the constant is imported and not re-spelled, and the arity,
order and operand meaning match the owner's helper:

- `LIABILITY_BASIS_MARKET_SEED_V2` ·
  `programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs:1013`
  — `[SEED, fixture.core_market]`, arity 2, against `LiabilityBasisMarketSeedsV2`.
  Arrived with `3c42f0ece`'s bump-mining of the campaign.
- `RAW_RECORD_PDA_SEED_V1` and `STAGING_CURSOR_PDA_SEED_V1` ·
  `programs/dclutch-trading-sbf/src/dealer/v3_equity_operator.rs:559`/`:568`
  (and again at `:1463`/`:1469` in its `#[cfg(test)]` module) —
  `[domain, REALM_SCHEMA_RELEASE_ID_V1, realm]`, arity 3, against
  `RecordPdaSeedsV1`. Arrived with `3c42f0ece`, which stopped that evaluator
  searching for eleven addresses.
- `STAGING_CURSOR_PDA_SEED_V1` ·
  `programs/dclutch-trading-sbf/src/admitted_composition_v3.rs:1396`, in the
  `#[cfg(test)]` module that pins the bump hint against the search.

`benign-sole-seed-no-helper` **×3** — the three rows that arrived `untriaged`
under the profile succession: `crates/dclutch-release-tool/src/infrastructure.rs:846`,
`successor/src/plan.rs:1257` and `successor/src/runtime.rs:1545`. Each imports
the domain from its owner and spells `&[DOMAIN]`; the owning crate still exports
no seed helper for it, so the standing fix the debt tag promises does not exist
here either. (Its length assert does exist again, as of this run.)

`inventory-guard-present` **×2** —
`crates/dclutch-operator/src/delegated_custody.rs:67`, which refuses
`custody.realm == [0; 32]` **and** all-zero on eleven real `Pubkey`s one line
later, and `crates/dclutch-wallet-terminal-payout-operator/src/wire.rs:1315`
(`parsed == Pubkey::default()`), which had been sitting `untriaged`.

`inventory-nonzero-guard-not-pubkey` **×1** —
`crates/dclutch-wallet-terminal-input-operator/src/address_book.rs:212`. The
guard is `evidence.linked_basis_record_digest == [0; 32]`, a **content digest**;
`Pubkey::default()` does not appear in the file. Filing it as
`inventory-guard-present` would put a false sentence in the register, which is
the whole reason that tag exists.

### benign-built-meta-census

**2 entries.** `TRANSACTION_LEVEL_SIGNER_CENSUS` ·
`tools/gauntlet/journey/src/resolution.rs` `retire:959` and
`tools/local-validator/bootstrap/successor/src/sponsored_push.rs`
`admit_terminal_instruction:1697`.

Both refuse `.accounts.iter().any(|meta| meta.is_signer)` — and the subject is
an `Instruction` an operator builder **just returned to this function**, so the
`is_signer` being read is an `AccountMeta` flag the builder wrote, not an
`AccountInfo` privilege the runtime assigned. The class's harm statement is
about the second: *the fee payer reads true here whatever meta it was given*.
It cannot occur over a `Vec<AccountMeta>` that has never been in a transaction;
privileges merge at message assembly, which is downstream of both of these
reads, and refusing here produces a driver error before a transaction exists —
the same structure `benign-declared-privilege-census` records for
`parse_route_v1`'s JSON document.

The bar `checked-caller-excludes-payer` sets applies and lands on the other
side of it: *if no builder can put a privileged account in the frame, the entry
is a false positive and belongs in a `benign-` tag instead*. One named builder
each (`build_resolution_direct_close_fund_v1`,
`build_resolution_admit_terminal_v3`), both deterministic over a finalized
snapshot, and in both drivers the payer is a separate value
(`payer: &Keypair`, the sponsor's `signer`) that is never placed in the frame.
The assertion these two make is the route's own permissionless claim — *"the
direct Resolution close is supposed to be permissionless and unsigned"*, *"the
fee payer is not an authority"* — and it is exactly the assertion that should
be red if a builder ever starts requesting a signature.

**Not admitted as a reader change.** The obvious generalisation — teach class 6
that `AccountMeta` is a request and `AccountInfo` an observation — would also
retire `crates/dclutch-operator/src/direct_close_maker_v1.rs` `assemble_plan`,
whose census over `meta_closure.accounts` is `checked-caller-excludes-payer`
precisely *because* someone asked whether a builder of that route places a
signing account in the frame and the answer changed the code. A reader that
cannot see the difference between "no builder can" and "the builder was
fixed" should keep asking.

### What was measured, and what was not

`tools/ci/run.sh seam` PASS at `HEAD` from a detached worktree; the register
carries **zero** `untriaged`. `cargo check` green for `dclutch-custody-sbf`,
`dclutch-registry::release_set`, and the `affine-batch` and `fractional-atomic`
program-test workspaces in their own target directories. The seam-audit's 32
machinery tests pass and both register tests, which were failing on the four
`untriaged` rows, now pass.

**Two debts named and not paid.** `d8a679168` changes an SBF link and
**leaves the frameguard ratchet red** — two `#[inline(never)]` functions leave
Custody and one arrives, so frames move, and the double build that would capture
the rows is longer than this tree's interval between program commits.
And `tools/fractional-exterior` does not build at `HEAD` and did not before this
run: `src/stage.rs:216` omits `raw_bump` and `staging_bump` from a
`NarrowRecordV2` that `b312ce3c4` widened. Neither is this lane's, and neither
is hidden by it.

### Addendum, same day — two negative controls had gone stale, and one of them was hiding a reader hole

**A checker that has never caught a known defect is decoration**, so the three
readers this run changed were held to their own controls. Two of the twelve were
already failing at HEAD, both because the tree moved under them, and neither
failure was caused by this run's changes. (Twelve, not the eleven the README
said — the count went stale when `AUTHORITY` was added on 2026-09-01, and the
whole suite is nearer nine minutes than four on this machine. Both corrected.)

**`DOMAIN_BYTES_COLLIDE` was a `live` control on a defect that had been fixed.**
It required the reader to name the `CLAIMS_FOUNDING_AGGREGATE_SEED_V4`/`_V5`
collision at HEAD, and `b209be565` answered it in the source. Re-filed as
**historical** against `b209be565`, which is a *stronger* bar than the live one
it replaces: it now requires the reader to fire at the parent **and** to be
silent on the fix.

**`AUTHORITY_CACHE_UNDERIVED`'s synthetic control had stopped mutating
anything.** It replaced
`authenticate_activation_cache_bump_v1(registry, cache, &request.release_set)`
in Custody's `authenticate_market`, and `5709672aa` deleted that call when it
made the route decode the cache once. A string replace that matches nothing
leaves the tree untouched, so the control read `0 before, 0 after` — the exact
shape of *distrust silent success*. The mutation now deletes the
`authenticate_activation_cache_identity_v1` delegation where it actually lives,
and **raises rather than skips** if that spelling ever stops matching.

**Retargeting it exposed a hole in the reader, which is repaired here.** With
the delegation deleted, the class stayed silent — because its one level of call
resolution was **account-blind**. It cleared any function that called a local
callee which derived an address and checked an owner, and Custody's
`authenticate_market` derives the *Market* address and owner-checks the *Market*
account. So a function that had lost its cache authentication entirely was
cleared by a callee that had never looked at the cache. The one-hop clearance
now also requires the callee to name the cache's own coordinates —
`ACTIVATION_PDA_DOMAIN_V1`, `ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1`,
`ACTIVATION_CACHE_BUMP_OFFSET_V1`, `cache_bump`.

Deliberately **not** `ActivatedExecutionReleaseSetViewV1`: that name is in the
signature of every function taking an already-decoded view, `authenticate_market`
included, so keying on it would put the hole straight back. Both legitimate
one-hop authenticators the README names still qualify — the Registry's
`authenticate_cache_identity` and Trading's
`require_activation_cache_account_v3`, each of which reproduces the cache
address from its carried bump.

Measured: silent at HEAD before and after the tightening (the gate does not
move), and the synthetic mutation now produces exactly two findings —
`authenticate_reservation_frame_v1` and `process_instruction`, the two functions
whose delegation it deleted.

12 controls, 12 PASS — the eight non-`PRIVILEGE` rows and the four
`PRIVILEGE` ones measured in two runs, because the suite outruns a ten-minute
budget in one.
