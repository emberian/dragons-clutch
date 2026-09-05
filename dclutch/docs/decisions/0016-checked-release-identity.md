# Decision 0016: a checked release describes source, artifact and account — three facts, three authors

Status: **ADOPTED 2026-08-30 — option A, plus the 0012 residual.** A checked
release describes the source by `semantic_release_id`, the artifact by the ELF
digest, and the account by a policy the live observation must satisfy: three
facts, three authors, no self-reference. `dclutch-release-tool` **stays
strict** — an iteration substrate is named, never defaulted into
(`DECISION_PACKET_2026_08_30.md` §2, orchestrator ruling with ember's veto
window open; `27f7944b`). M-25 closes with this record.

Ledger M-25, as originally posed: **recording requested, not re-litigation.** The
question was flagged 2026-08-27 and *"disowned with no recipient"*; the work of
08-27 → 08-30 answered it in practice. This record proposes writing the answer
down and ruling the one residual it leaves.

## 1. The question

> **Consequence, and it is not mine to resolve:** an offline `loader-accounts`
> construction cannot represent a revoked ProgramData, so a checked release
> over one can never match the account. Whether a checked release describes
> the **artifact** or the **account** is a `dclutch-release-tool` question.
> Reported, not patched.
> — `docs/evidence/FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md:199-203`

The framing assumed the answer had to be one or the other. It is neither: the
tree binds **three** identities, each with exactly one author.

## 2. What the code does today

**Layer 1 — SOURCE: `semantic_release_id`.** Source-derived, and the stable
fixed point: *"set-id moves every rebuild; Trading/Resolution held one semantic
id across four cohorts while ELFs/slots moved, test-pinned"* (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:420-423`,
the ERA lane). Clients select frames by on-chain release identity keyed on the
per-role semantic id and follow the chain to the current activation cache, with
incoherence a hard refusal. It is enforced on-chain by
`authenticate_role_semantic_release`
(`crates/dclutch-resolution-core-v3-operator/src/lib.rs:2596, 2603`).

**Layer 2 — ARTIFACT: the ELF digest.** Bound in the `ArtifactReleaseV1`
record, written at `ELF_DIGEST_OFFSET` alongside the semantic id
(`crates/dclutch-registry-contract/src/artifact.rs:160-182`). The 2026-08-29
ruling deliberately kept these two apart (`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:1166-1174`):

> the certificate binds `semantic_release_id` (source-derived, the identity ERA
> proved stable across cohorts and that `authenticate_role_semantic_release`
> already refuses on on-chain), NOT `elf_digest`. The ELF digest stays bound
> where it already lives — the `ArtifactReleaseV1` record — so the end-to-end
> guarantee is **two facts with one author each instead of one self-referential
> fact**.

That ruling has a measured cause: a certificate binding its own ELF digest is
**unconstructible**, because its identity is compiled into the ELF whose digest
it must contain — proven by two builds differing only in the certificate, with
the dead-code-elimination escape ruled out by byte count (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:344-351`).

**Layer 3 — ACCOUNT: a policy the observation must satisfy.** Decision 0012.
`ArtifactReleaseV1::to_bytes` writes the `upgrade_policy` byte, the
`deployment_slot`, and the bound `upgrade_authority` into the record
(`artifact.rs:169-182`), so all three move every PDA the Registry derives. The
release does not describe the account's *state*; it states a **policy** —
`Immutable`, or `ExactAuthority(authority) + deployment_slot` — and
`require_slot_pinned_release_v1` refuses an observation that disagrees. The
soundness argument is 0012's: every Loader V3 `Upgrade` writes the current slot
into ProgramData, so observed slot equality implies the bytes are the bytes
first admission hashed.

**The distinction is observable on-chain, and the slot pin is the bridge.** In
the uncached path the deployed bytes are actually hashed
(`hash(programdata_view.elf())` in
`crates/dclutch-shadow-accelerator-auth-v4/src/deployment.rs`); on the cached
path an `ExactAuthority` release may reuse the activation-bound digest *because*
the slot matched, measured at **+73 CU**
(`docs/decisions/0012-devnet-iteration-substrate.md:97`).

## 3. The specific defect the flag named is closed

The flag's real content was narrower than its headline: an offline
`loader-accounts` construction writes an all-zero authority tail, while a
genuinely revoked ProgramData keeps *"the old key sitting inert at bytes 13..45
behind a zero tag"* — same length, one 32-byte window apart
(`FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md:185-197`, measured against deployed
Core).

Three things closed it, all after the flag was written:

- **The plan pins the post-revocation bytes separately** from the genesis
  digest — `post_revoke_programdata_sha256`, checked in four places
  (`tools/local-validator/bootstrap/successor/src/runtime.rs:1581, 1745, 1910,
  1934`), with its own comment calling it *"a stronger check"* (`:1922`).
- **The host mirrors were converted** (decision 0012, POST-0012): `localSuccessor.ts`
  carried `requireZero(bytes, 13, 32)` on the ProgramData header — the check
  `releaseRegistry.ts` had removed the same day after a live measurement — and
  it *"passed only because the local genesis writes that tail zeroed, so it
  would have failed on the first revoked role read from a real cluster."*
  Recorded as Blocker D in `docs/design/DEVNET_DEMO_DEPLOY.md` §7.
- **Checked manifests now name their substrate.** Deviation 3 closed as a
  manifest *field* rather than a decode flag (commit `7bb9a075`): both checked
  manifests carry a derived `evidence_class` naming which substrate they are
  evidence for, with the
  `loader-state-carrying-an-observed-retained-authority` precedent as its model.

So a checked release over a revoked ProgramData is representable today, and the
tool says which substrate it is evidence *for* rather than silently accepting
either.

## 4. Options

| option | cost | consequence |
|---|---|---|
| **A. Record the three-layer answer as a decision** | this document | The next reader stops re-deriving it. M-25 closes. |
| B. Re-open the artifact-vs-account choice | a lane | There is nothing to choose: the three bindings exist, are load-bearing, and each has a separate proved or measured reason. |
| C. Add an explicit authority-state field to the release descriptor | small | Redundant — the `upgrade_policy` byte plus the bound authority already is that field, and unlike a state snapshot it cannot go stale, because it is a predicate the observation must satisfy. |

## 5. Recommendation

**Option A, plus one ruling.**

Record the answer in the form above: **a checked release describes the source
by `semantic_release_id`, the artifact by the ELF digest in
`ArtifactReleaseV1`, and the account by a policy the live observation must
satisfy.** Three facts, three authors, no self-reference. The flag's dichotomy
was false, and saying so is worth more than the flag was.

**The residual that is genuinely ember's:** decision 0012's PIN-0012 yield
flagged three deviations for veto, one of which is still open and belongs
here — **`dclutch-release-tool`'s strict gate was left strict deliberately**
(`docs/decisions/0012-devnet-iteration-substrate.md:14-18`). Now that a
checked release can name a mutable substrate through `evidence_class`, the
question is whether the tool should keep refusing one by default. Recommend:
**keep it strict, and let the iteration substrate be named rather than
defaulted into** — the same discipline gen-1 imposed on zero-fee and the
project imposes everywhere else. Ember should say so or override it, because
it is the last piece of the 0012 veto window nobody closed.

## 6. What changes downstream once ruled

- M-25 closes with a record rather than a fourth reader re-deriving it from
  `WAVE.md` ruling text and a stale evidence line.
- `FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md:199-203` gains a pointer here so the
  flag stops reading as open.
- The release-tool strict-gate ruling closes decision 0012's veto window.
- One debt stays named and is not discharged by this record: `deployments.ts`
  still carries a stale constant, non-load-bearing since the ERA lane made
  clients follow the chain, and it wants a publish-time owner (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:424-427`).
