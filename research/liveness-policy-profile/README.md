# Liveness policy evidence profile

Status: **MEASURED INTERMEDIATE EVIDENCE / ARITHMETIC CANDIDATE / NOT
PROMOTABLE / FINAL ELF PENDING**.

This directory binds the local-bank CU samples and account-rent inventory used
to exercise `crates/clutch-liveness`. It does not integrate that kernel into the
SBF runtime, promise transaction inclusion, select a neutral failure sink, or
convert any token or Hoard collateral into SOL.

The evidence/test tree is commit `a29902b`; its program boundary is `3a81b38`.
The copied intermediate SBF artifact is exactly
`c8ff4ac7286004cb5d897cc92b05f7a9e386107d295cb1441adcd227e0b35138`
(`809824` bytes). `policy.py` reconstructs that historical source tree before
compiling the account probe, so later ABI drift cannot silently rewrite the
measured rows. Occupation-v4 integration is still allowed to supersede this ELF;
the profile must be refreshed rather than called final if it does.

Run the reproducible checks:

```sh
python3 research/liveness-policy-profile/policy.py
python3 -m unittest discover -s research/liveness-policy-profile -p 'test_*.py'
cargo test --offline --locked --manifest-path research/liveness-policy-profile/Cargo.toml
cargo clippy --offline --locked --manifest-path research/liveness-policy-profile/Cargo.toml --all-targets -- -D warnings
```

`--check-current` is a deliberately strict drift gate. It fails if any measured
test/layout source or the current account probe differs from `a29902b`. Use it
only to ask whether the historical profile still describes the live working
tree; a failure means “remeasure,” not “edit the expected value.”

`--replay` materializes the pinned source tree and reruns the selected bank
campaign, including the joined degree-one through degree-three native lifecycle,
against the exact copied ELF. It first checks the ELF digest and sets
`SBF_OUT_DIR` explicitly. This avoids silently loading either the older
checked-in fixture or a concurrently rebuilt `target/deploy` artifact.

The reviewed interpretation is in
[`docs/implementation/LIVENESS_POLICY_PROFILE.md`](../../docs/implementation/LIVENESS_POLICY_PROFILE.md).
