# Liveness policy evidence profile

Status: **R1 ARTIFACT SEALED / MEASURED RESOLUTIONWORK / TERMINAL INVENTORY
CHECKED / PROTOCOL ADMISSION STOP**.

This directory contains:

- `admission_math.py`: fail-closed CU quotes and staged ResolutionWork/Direct
  path maxima;
- `terminal_admission.py`: strict account/value terminal checker;
- `terminal_profile.py`: complete current-runtime account classification;
- `src/main.rs`: exact account-width and pinned-default-rent probe;
- `policy.py`, `evidence.json`, and the normalized capture: exact artifact,
  bank, source/test identity, rent, reward, and source-drift seal;
- `artifacts/a5725a3d8e149b2b`: the canonical ELF, ordinary and relocated build
  logs, stack/ELF audit summaries, bank logs, upstream report, and checksum
  ledger.

The profile never treats Hoard principal, fees, future volume, a future
subscriber, a token price, or a token-to-SOL conversion as liveness funding.
It publishes no finite work quote when the requested CU headroom fails and no
complete `LivenessPolicy` tuple while any mandatory path remains stopped.

Run the exact seal, strict current-runtime drift gate, and stable arithmetic
and terminal checks:

```sh
cd research/liveness-policy-profile
python3 policy.py
python3 policy.py --check-current
python3 -m unittest -v \
  test_policy.py \
  test_admission_math.py \
  test_terminal_admission.py \
  test_terminal_profile.py

cargo run --offline --locked \
  --manifest-path Cargo.toml
cargo clippy --offline --locked \
  --manifest-path Cargo.toml \
  --all-targets -- -D warnings
```

The artifact source is exact commit `7e8f6b1`; the corrected test-fixture and
release ancestry is exact commit `316c620`. The profile keeps those identities
separate. Native full-lifecycle tests are intentionally excluded from the
default feature: running them requires the distinct non-production mock-source
ELF, so they are not smuggled into this projection.
