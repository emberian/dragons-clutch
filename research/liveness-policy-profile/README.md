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
- `artifacts/af6bb79cc3766bd0`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/bd20711b01828a74` and `artifacts/a5725a3d8e149b2b`: the preceding
  historical seals, retained in full for audit continuity but excluded from the
  current projection. `policy.py` refuses a seal that overwrites a superseded
  artifact root or drops any of its evidence files.

Every sealed path is checked for repository membership, not merely for
presence on the running disk. The root `.gitignore` excludes `*.so` and
`*.log`, so a plain `git add` of a new artifact root silently commits a
fraction of it while every hash of a working-tree file keeps passing;
`check_tracked_evidence` therefore requires each current and retained
historical evidence path to be tracked and to equal its committed blob at
`HEAD`, refusing an ignored, staged-but-uncommitted, or
modified-after-commit file. If git cannot answer that question the checker
reports `UNAVAILABLE` with the exact git failure and exits nonzero; an
unanswerable question is never reported as tracked.

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

The current artifact source and test/evidence ancestry is exact commit
`2d530d2`. The preceding `bd20711b…` artifact remains historical only: the
Direct V3 selection lifecycle merge grows the declared runtime closure from 88
to 94 files and the stripped ELF from `1,228,192` to `1,490,544` bytes. Exact
ELF comparison finds no byte-identical section except `.dynstr` and
`.shstrtab`, so this is a materially different artifact and no old CU row was
reused as current-artifact evidence; every measured row was rerun against exact
`af6bb79c…`. Direct V3 is resident but unmeasured here, so no V3 row enters the
projection. Native full-lifecycle tests are intentionally excluded from the
default feature: running them requires the
distinct non-production mock-source ELF, so they are not smuggled into this
projection.
