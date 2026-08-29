# Controller-funding split: real-SBF adversarial evidence

Date: 2026-08-28  
Scope: `c10437b6`, `f4b456c9`, and the decision-0012 cleanup admission change

The original single-transaction abort cannot execute both controller closes.
A Resolution-first close consumes about 1.35 million CU before Trading can
close its ledger; a Trading-first close violates Solana's CPI balance boundary
when the outer program moves lamports from an account owned by Trading before
the Resolution CPI. The durable repair is an authenticated prefix followed by
an independently resumable suffix, not a larger atomic frame.

## Persisted protocol

`c10437b6` introduced the following exact cleanup state machine:

- phase 1 `Prepared` and phase 2 `CustodyStaged` remain the only Open inputs;
- `DCLTPCA1` moves an expired Custody-staged checkpoint to phase 3
  `CustodyAborted` after the Custody abort completes;
- `DCLTCF1A` closes the canonical first controller ledger and then persists
  phase 4 `PreparedFirstLedgerClosed` or phase 5
  `CustodyFirstLedgerClosed`; and
- `DCLTCF2A` reauthenticates the exact prefix receipt and remaining live-ledger
  digest, closes only that remaining ledger, and closes the checkpoint last.

The account widths are 35 for `DCLTPCA1` and 17 each for `DCLTCF1A` and
`DCLTCF2A`. The terminal `DCLTCFR2` receipt is 512 bytes. It binds the cleanup
origin, canonical controllers and masks, phase and revision, program and
checkpoint identities, prior checkpoint digest, both ledger prestate/closed
state/child-receipt digests, immutable principal and Rent destinations, exact
amounts, checked totals, and first/final slots. `f4b456c9` moved the enlarged
checkpoint authentication and Custody persistence into callee-owned frames.

## Decision-0012 trust boundary

Activation authenticates the complete ELF digest and persists the checked
`ArtifactReleaseV1` in the activated execution-release-set cache. Cleanup does
not hash the two large ELF tails again. It parses the live Loader V3 Program and
ProgramData accounts and requires all of the following to equal the activated
release: Program identity, Loader owner, executable disposition,
Program-to-ProgramData link, derived ProgramData key, ProgramData owner and
non-executable disposition, deployment slot, and upgrade-authority policy.
The cleanup request also binds the exact activated release-set identity, whose
decoded cache projection authenticates every role and artifact-release ID.

The admitted ELF digest is recovered only after the live ProgramData slot and
authority match the release that was fully hashed at activation. A different
slot, key, link, owner, executable bit, authority, cache body, role, or release
set therefore refuses before either ledger mutates. This is full-digest
authentication at activation and slot-pin authentication at recurring use; it
is not a claim that slot equality hashes program bytes.

## Real-SBF result

The focused fixture is
`crates/dclutch-svm-harness/tests/controller_funding_split_abort.rs`. It runs
the production Trading, Resolution, and Registry ELFs in ProgramTest at the
1,400,000-CU ceiling. The exact command was:

```text
SBF_OUT_DIR=/tank/dregg-build/dclutch-split-cleanup-c104-evidence/sbf-out \
CARGO_TARGET_DIR=/tank/dregg-build/dclutch-split-cleanup-c104-evidence/target/harness \
swarm-build cargo test \
  --manifest-path crates/dclutch-svm-harness/Cargo.toml \
  --test controller_funding_split_abort --offline -- --test-threads=1
```

Result: **6/6 passed**. The focused builds emitted zero SBF frame diagnostics
for both Trading and Resolution. Artifact SHA-256 digests were:

| program | SHA-256 |
| --- | --- |
| Trading | `bfe9594ecc8affae09440e06377057210450c1cd9a7dfab280359441f9854792` |
| Resolution | `d7d1f709d183dd72db96bac54105da170d107953575963e598c1cc0a95877c61` |
| Registry | `bd7b767dbc03fc3d07ee42965834447ae90346673a1b15547044a3750c026726` |

Successful transaction consumption was:

| canonical order | first transaction | terminal transaction |
| --- | ---: | ---: |
| Resolution then Trading | 184,213 CU outer; 138,826 CU Resolution child | 58,696 CU |
| Trading then Resolution | 58,352 CU | 209,336 CU outer; 149,849 CU Resolution child |

The two order tests prove an expired prefix persists, replay of the first
transaction refuses, a +1-lamport remaining-ledger substitution refuses with
the prefix byte-for-byte unchanged, the exact suffix then resumes, terminal
replay refuses, checkpoint closes last, principals return only to the immutable
funding source, all three exact Rent reserves return only to `RentCredit`, and
no lamport is stranded or changes classification. Separate tests prove
before-expiry refusal and that the Prepared route cannot consume phase 2.

The slot-pin hostiles are pre-genesis fixtures rather than loader-cache edits:
an activated deployment slot of 1 against live ProgramData slot 0 refuses with
`0x8006`; an activated exact authority against an immutable live ProgramData
refuses with `0x8006`; and a substituted activation-cache body refuses with
`0x8005`. Every refusal preserves the checkpoint, both ledgers, funding source,
and `RentCredit` exactly.

## Remaining vertical gap

This fixture starts from a real `Prepared` graph. It does not claim a full
real-SBF `DCLTPCA1` Custody abort: that requires the complete four-account
Custody ladder and belongs in the successor/private-validator lifecycle. The
phase-confusion refusal is covered here, and capability-contract tests cover
the exact Custody digest and phase transition, but neither substitutes for the
vertical Custody CPI. The successor also still owns durable transaction
journals and live-message unique-lock census; this program fixture does not
claim either result.
