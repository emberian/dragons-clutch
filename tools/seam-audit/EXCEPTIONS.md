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
Main moves under this file. What the baseline pins is the finding *set*, not
a revision: if it still reproduces exactly, the gate is green wherever main is.

---

### confirmed-defect

**5 entries. Open defects, all reachable, none fixed here.** These are recorded
so the gate does not report the tree as clean while they stand — not excused.

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
  `dclutch:lbv2:market`, across `crates/dclutch-claims-svm` and three
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

- `DOMAIN_BYTES_COLLIDE` · `PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2` +
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
(`crates/dclutch-record-contract`), a compile-time length guard, and 107
consistent three-seed derivation sites. The name/bytes gap here is stylistic.

### debt-seed-guard

**61 entries.** A PDA seed domain within 32 bytes today with no compile-time
assert holding it there. Not a live defect — every one of these derives an
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

**239 entries.** A seed tuple spelled out in a crate that does not own the
domain, where the owning crate exports a seed constructor for exactly that
purpose. Each is a second author for one address.

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

**28 entries.** A blanket `is_signer` refusal over a whole account frame,
minus the one already confirmed as #13b.

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

**24 entries.** An exact writability census over every coordinate of a frame
with no exemption anywhere.

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

**257 entries. Not findings.** Each records that one file refuses the unset
pubkey somewhere in it. The class has no defect behind it, so the only useful
property to assert is that the existing guards stay: the gate's ratchet turns
both ways, so a file quietly losing its last guard is reported as `GONE` and
fails.

Keyed by file rather than by function on purpose. Function-level keys
inventoried 586 guards and would have failed the gate twice — once `GONE`, once
`NEW` — on every rename of a guarded function. A ratchet nobody can live with
gets switched off, and then it guards nothing.
