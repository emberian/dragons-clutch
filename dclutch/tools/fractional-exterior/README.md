# Fractional exterior

This tool makes the Fractional lifecycle observable at the process boundary of
a real `solana-test-validator`. It uses deterministic test identities only. It
does not read a wallet, sign with a user key, deploy, or address mainnet/devnet.

There are two complementary campaigns:

- `run` starts at a live curved degree-3 Product and executes wrap, a real
  Token-2022 transfer to an independent sleeper, and whole-claim unwrap.
- `run-claim-check` starts at the canonical output of permissionless terminal
  compaction and executes the durable holder life: hostile rollback, partial
  burn/pay, settling burn/pay, record closure, and permissionless escrow close.

The real-ELF ProgramTest campaign at
`programs/dclutch-claims-sbf/program-test/fractional-atomic/` owns the seam
between those two boundaries. It executes the curved terminal payout and
permissionless sleeping-holder compaction rather than planting their result.
The second private-validator campaign deliberately stages that result so it can
exercise the public post-retirement protocol without depending on extinct
Market/Trading accounts.

The opening Product is the clamped cubic `[0,0,0,0,3,3,3,3]`. At the
authenticated coordinate `3/2`, its exact weights are `[1/8,3/8,3/8,1/8]` and
the protocol's one cumulative-floor boundary at scale 11 yields `[1,4,4,2]`.
The exterior represents coordinate 1 with denominator 10. Seven native claims
therefore wrap into 70 shard atoms; 20 move to an unrelated holder; and the
remaining 50 burn during unwrap and unlock five native claims. Exactly 20 shard
atoms and two reserve claims remain for the sleeper, with every other Product
coordinate unchanged. The matching terminal campaign pays four collateral
atoms per whole claim at that same coordinate.

## Claim-check commands

The ELF directory must contain exactly the runtime artifacts named below:

```text
dclutch_claims_sbf.so
spl_token_2022.so
```

Both paths must be absolute:

```sh
cargo run --manifest-path tools/fractional-exterior/Cargo.toml -- \
  run-claim-check \
  --elf-dir /absolute/path/to/elves \
  --out /absolute/path/to/evidence

cargo run --manifest-path tools/fractional-exterior/Cargo.toml -- \
  verify-claim-check \
  --out /absolute/path/to/evidence
```

`run-claim-check` owns RPC port `20962` and stops its validator unless `--keep`
is present. `prepare-claim-check` writes the same genesis accounts without
starting a validator.

## Exact claim-check life

The canonical post-compaction input is 70 shard atoms with denominator 10,
backed by 28 collateral atoms at 4 collateral atoms per whole claim. There is
one rounding boundary:

```text
whole_claims = floor(requested_shard_atoms / denominator)
consumed_shards = whole_claims * denominator
```

The submitted action order is fixed:

1. Dust (`9`) refuses with `0x5665`.
2. Overdraw (`71`) refuses with `0x5663`.
3. A payout account owned by another actor refuses with `0x5661`.
4. A substituted holder signer refuses with `0x5661`.
5. Partial redemption burns `20` shards and pays `8` collateral atoms.
6. Premature escrow close refuses with `0x5625`.
7. Settlement burns `50` shards, pays `20` atoms, closes the record, and
   returns every live record lamport to the holder.
8. An unrelated actor closes the zero-balance vault and settled escrow and
   receives their complete live rent balances, less only the transaction fee.

Every refusal compares the complete Claims and Token-2022 account snapshots
before and after. Every accepted action checks Mint supply, holder and vault
token atoms, record balance/absence, escrow outstanding count/absence, and
lamport destinations.

## Evidence contract

The output directory contains:

- `claim-check-manifest.json`: exact ELF SHA-256 digests and deterministic
  account/PDA identities.
- `claim-check-canonical.json`: exact instruction-data and ordered account-frame
  digests, accepted/refused result, named refusal code, and complete protocol
  poststate for all eight actions. Its digest is byte-identical across reruns
  with the same artifacts.
- `claim-check-observed.jsonl`: transaction signatures/details, wire sizes,
  fees, and actual closer balances. These cluster-volatile facts are excluded
  from the canonical digest.
- `claim-check-validator.log`: validator/runtime logs.

`verify-claim-check` reconstructs every instruction and account frame from the
sole wire authorities, compares every poststate and refusal, binds the journal
to the staged ELF digests, and rejects missing or additional canonical fields.
Static JSON is evidence, not protocol authority.

## Propagated cubic-life commands

`run-cubic-life` is the one resumable campaign that carries one cubic
Fractional identity through all three public boundaries: preterminal
wrap/transfer/whole-unwrap; real-ELF terminal, permissionless compaction; and
the post-compaction hostile/partial/settling/close life. It does not substitute
a separately invented claim-check fixture for the compaction output.

Its complete argv contract is exactly one of the following (the three named
options may be supplied in any order):

```sh
cargo run --manifest-path tools/fractional-exterior/Cargo.toml -- \
  run-cubic-life \
  --source-root /absolute/path/to/dragons-clutch \
  --elf-dir /absolute/path/to/elves \
  --out /absolute/path/to/evidence

cargo run --manifest-path tools/fractional-exterior/Cargo.toml -- \
  verify-cubic-life \
  --source-root /absolute/path/to/dragons-clutch \
  --elf-dir /absolute/path/to/elves \
  --out /absolute/path/to/evidence
```

Every value is required and absolute. The life commands accept no `--keep`, no
other option, and no duplicate option; an unknown flag, duplicate flag, missing
value, or relative path refuses. `--source-root` must be a clean Git worktree
at a full committed `HEAD`; the command records that commit and the SHA-256 of
its complete `git ls-tree -r --full-tree HEAD` output. The ELF directory must
provide these exact executable artifacts: `dclutch_claims_sbf.so`,
`dclutch_registry_sbf.so`, `dclutch_core_sbf.so`, `dclutch_custody_sbf.so`,
`dclutch_rent_sbf.so`, `dclutch_fractional_compaction_test_caller_sbf.so`, and
`spl_token_2022.so`.

The immutable propagated identity is the preterminal bridge's source commit,
source-tree digest, seven ELF SHA-256 pins, release set, Realm, Market,
aggregate, Product, ProductBasis, terms, Fractional root, shard Mint, sleeping
holder and holder shard account. It additionally fixes the cubic degree `3`,
payout scale `11`, representation coordinate `1`, denominator `10`, `40`
outstanding shards, and `4` reserve native claims. The sole named integer
rounding boundary is:

```text
whole_claims = floor(shard_atoms / denominator)
collateral_atoms = whole_claims * payout_per_claim
```

The compaction bridge embeds that entire preterminal bridge rather than
restating it, commits to its canonical SHA-256, and fixes payout `4` and escrow
collateral `16` atoms. It also supplies the exact persisted record, escrow,
vault, Mint, and token-account images that seed post-compaction validation. The
post-compaction journal is bound to the compaction-bridge digest, so a signer,
Claims PDA, account owner/image, amount, source, or ELF substitution refuses.

`run-cubic-life` writes or resumes this layout under `--out`:

- `01-preterminal/`: private-validator `canonical.json`, volatile
  `observed.jsonl`, `manifest.json`, validator log, and genesis account files.
- `preterminal-bridge.json`: canonical preterminal identity and its
  preterminal-journal digest.
- `02-compaction-bridge.json`: canonical real-ELF compaction result, embedding
  and committing to `preterminal-bridge.json`.
- `03-postcompaction/`: propagated claim-check canonical journal, volatile
  observations, manifest, validator log, and genesis account files.
- `fractional-cubic-life-v1.json`: canonical three-phase ledger, including both
  bridge digests, both journal digests, identity fields, payout/conservation
  values, and the completed phase list.

Bridge and ledger JSON must decode with no unknown fields, validate their fixed
amount/identity contracts, and be byte-for-byte canonical pretty JSON with one
trailing newline. Bridge and final-ledger writes use a same-directory temporary
file, `sync_all`, and rename; an encode failure cannot replace an accepted
file. A rerun first verifies each already-present boundary: preterminal journal
against bridge; compaction's embedded preterminal; then the propagated
post-compaction journal. If the final ledger already exists, it verifies the
entire campaign and makes no new run. `verify-cubic-life` independently repeats
these source, ELF, bridge, journal, canonical-byte, and ledger checks.

The post-compaction phase records four hostile redemption refusals (dust
`0x5665`, overdraw `0x5663`, substituted payout account `0x5661`, and
substituted holder signer `0x5661`), followed by partial burn/pay, a premature
close refusal (`0x5625`), settling burn/pay, and permissionless close. Every
refusal preserves the complete Claims and Token-2022 snapshot; accepted steps
check supply, balances, record/escrow state, and rent destinations.

This is local-validator evidence with real ELF artifacts and a real-ELF
ProgramTest compaction boundary. It is not mainnet evidence, a deployment, or
formal verification.
