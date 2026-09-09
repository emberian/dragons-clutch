# Retained Structured and Series host recovery — 2026-09-09

These are actual local-validator observations and scoped host controls. The
retained eight-program runtime is source
`691bc6ff0ab1c89db6f8ab307e6ff1d75e0557a6`; it remains in place. Host corrections
were executed only in isolated V1 descendants, never by placing a V2 host over
that runtime. No public deployment is established by this evidence.

## Structured retained continuation

The live directory is
`hbox:/tank/dregg-build/structured-validator-32991-691-20260909`, RPC port 28783.
Its explicit external Token genesis is recorded in
[the genesis fidelity evidence](STRUCTURED_TOKEN_GENESIS_FIDELITY_2026_09_09.md).
The original receipt activation accepted at slot 12253, 400608 CU, signature
`4RvZQEZDqugVGoxWPV8SmuTVaJQsQWCxddcAgaqVCfDm6ns6KGJfUfZVFCAm7ZJouXzqFwvnojRrQQNfBpLJ5cfk`.
The terminal host entrance reuses that campaign and checked gate; it does not
restart the validator or republish a replacement market.

The canonical operator accepts the exact coordinate caller signer at Claims
child coordinate 20 only for ActivateCoordinate/RetireCoordinate. Its outer
profile still requests only actual wallet signatures. The old blanket child
rule reproduced `ChildFrame`; the production selected-frame control passed
with the native coordinate signer and refused missing callers or an unrelated
extra signer. Source commit: `01e7b052f`.

The retained continuation entrance (`8db90b7d1`) and split rent prepayment
(`fbcc14d32`) advanced beyond that host wall. The earlier combined lifecycle
packet exceeded the packet limit. Rent now comes from the campaign payer in a
separate transaction, followed by finalized observation; preexisting balances
are preserved and only deficits transferred.

At slot 20627, signature
`5TURgYHKQT8Rf3ZVLYqNfu1DpLNbLromHMdgwKesHJ4E3UwcwnC5vMjoSfkb9wt3K1SQ4QnVkWQcYaistNTz7n1x`,
the payer spent 11120520 lamports: four canonical resources gained
2547360, 2004480, 4454400, and 2039280 (11045520 total); the fee was 75000.
The sum of account deltas equals the negative fee. All four accounts remained
System-owned with empty data after the following simulation refused. The full
transaction and poststates are in
`coordinate-0-prefund-observation.json` in the retained directory.

`continuation-2f2.log` ends at simulated coordinate activation refusal
`TradingSbfError::Content`, 235915 total CU, before Claims CPI. Native fixes and
controls are recorded separately in
[the coordinate boundary evidence](STRUCTURED_COORDINATE_NATIVE_BOUNDARY_2026_09_09.md).
This retained run does not establish redemptions or retirement.

## Series source geometry

The retained directory is `hbox:/tank/dregg-build/s691-series`, RPC port 23816.
All diagnostic hosts use
`hbox:/tank/dregg-build/structured-host-a93c5c12d-completion/source` with that
checkout's own target. The diagnostic mode exits before parent founding.

Actual execution located these host defects, now corrected in the canonical
integration branch:

| Commit | Correction |
| --- | --- |
| `7eb4d69ad` | Prepay canonical M0 Claims rent and observe actual vacant System balances. |
| `265628c97` | Future-only publication uses input generation 1, matching native occurrence 0 derivation. |
| `46d6058ab` | Include Realm Mint/Token in M0 observation and preserve prepaid Claims sources. |
| `4a9ee2d0b` | Use one parent-root source mapper; pre-founding root has its native eventual width. |
| `7f42497c5` | Admit canonical System ID only as a finalized NativeLoader source; reject default PDAs. |
| `609b2e05d` | Predict Prepare-owned Ticket replay before Prepare, require finalized owner afterward. |

The isolated controls all use `cargo check --locked`, filtered tests with one
thread, and `cargo build --locked --release -p dclutch-local-successor-bootstrap`
through `swarm-build`. Claims vacancy owner validation, parent prediction,
native System collection, and Ticket prestate each have a failing old-behavior
control followed by a passing correction. The full constructor fixture also
proves the emitted fixed account profile accepts its mapped geometry. This is
host evidence; it does not replace actual observation of an emitted manifest.

At slot 91912 the Series payer spent 9429240 lamports: the three canonical
Claims resources gained 2004480, 2895360, and 4454400 (9354240 total), with a
75000 fee. Signature:
`S2Ev7Kc3vySNtcW1SpQCcfyawgwnotDo1erad98CHKUZG2vM1PUKb6ErDnwdoV8WdK5bTZhQu8BqXsiFLTEyfhE`.
The full transaction and finalized System-empty poststates are retained in
`series-claims-rent-17a-observation.json`. No principal was diverted into rent.

The diagnostic-only scenario uses period 65536 and retry window 32768. These
are provisional capture allowances, not lifecycle defaults or protocol bounds.
For source `7a8cf5a5397df97a39b17dcbba664322b66f92ce`, the finalized Template
starts at 98498, its second occurrence at 164034, and retry-through values are
131266 and 196802. The 40 actual transactions spanned 97753..99003; founder
publication and rent prepayment consumed 505 slots after first_slot. Exact
records are in `series-diagnostic-7a8-observation.json`.

Prepare is permitted before scheduled_slot, but must be at or before
retry_through. Consume requires scheduled_slot <= now <= retry_through.
The diagnostic did not wait for the second occurrence or warp the clock.
A full lifecycle scenario must use measured publication headroom and a practical
recurrence interval; the diagnostic schedule is not promoted into canonical
scenario code.

## Measured diagnostic capture and remaining Prepare wall

Canonical `308e75dee` shares one native-System identity predicate between
Consume and Expire and observes exactly the campaign payer/collateral wallet
needed for refund routes. The Expire constructor and shared System controls
both passed (one test each, 965 filtered).

The retained V1 host `79daa409a9e96ad664e6851a859965a1904cd92a` observed the full
Consume and Expire widths, then Prepare hydration refused distinct projected
caller authorities being assigned one release alias. The exact final error in
`shadow-diagnostic-79d.log` names
`5sAnG6tt1UM4LTfRBMUowARfjNqJY1EjGHA8q5kzMMNQ` and
`C19TdCNVR9Tc9ymzowFNonuqDYRy9FG7HZfe9KcF5mzX`.
The native Prepare alias owner maps projected Open caller 53 to Initialize
caller 6, although Custody's caller seed owner includes the complete distinct
request digest. Its normal Custody caller coordinates 68, 81, and 97 are also
mapped to 6. This is a located source/runtime-construction wall; it was not
removed from the retained runtime.

An isolated diagnostic-only host descendant,
`88c3de5a0e8b65ffd36cb9459571a396643cb1ec`, captures immediately after canonical
Consume/Expire observation, through the existing source generator and
hostile-decoded diagnostic writer. The explicit environment selector is
`DCLUTCH_SERIES_UNPUBLISHABLE_GEOMETRY_CAPTURE`. It returns an error after the
capture, preserving exit 1 and the unresolved Prepare/no-parent-founding
boundary. This isolated host commit must not be merged or published.

Output directory:
`hbox:/tank/dregg-build/s691-series/shadow-diagnostic-88c`.

| Artifact | SHA-256 |
| --- | --- |
| `series_shadow_source_manifest.bin` | `94179c66c0fae90c4067f646fe30dc867acd620d67e5f44f7ca5fb36f1b200dd` |
| `series_shadow_generated.rs` | `0e0a573731b557b06eb2eb320ad39c3b2a973066508f2f5283ae3f48f9268ab9` |
| `accelerator_semantic_preimage.bin` | `4792f0716e52711a36d77e5750be45e5ce24eb56b523da63cb32658a8e68c45f` |
| `observed-action-geometry.json` | `f627c20a1aa60216ed031c669cefa86f2abe485b67b7097342f34f98684becb2` |

Independent inspection confirms DCLTSSM2 version/profile 2, 17628 exact bytes,
164 fixed Consume widths (108 nonzero), two occurrences, and one funding slot.
All manifest widths exactly match the observed-action JSON. Expire has 82
widths, 62 nonzero. The emitted certificate digest is
`974efc3b8f3df3bf594c9ab4d9d97816cff110e79f3c7db424c0e80e9a11455b`.
The directory carries `UNPUBLISHABLE_DIAGNOSTIC_ONLY` and an independent
`capture-inspection.json`.

The capture's 40 transactions spanned slots 117509..118758. Finalized Template
`Hp9N7ChZuWuqTcPnh3KN7Xp8fQvqT67ZJxGfysc8sd3B` schedules slots 118254 and
183790, with retry-through 151022 and 216558. Finalized record bodies and their
hashes at snapshot 119120 are saved in
`series-diagnostic-88c-observation.json` beside the output directory. The final
log preserves the explicit error and exit 1.

These widths include the retained V1 ProgramData lengths. The source manifest
must be rebuilt and checked against exact fresh V2 runtime geometry before
selecting an accelerator. This is a concrete bootstrap input, not acceptance of
Prepare, Consume, Expire, recurrence, redemptions, or retirement.
