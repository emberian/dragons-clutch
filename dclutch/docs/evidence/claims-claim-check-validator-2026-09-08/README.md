# Claims claim-check local-validator execution — 2026-09-08

This corpus records eight finalized transactions against a fresh private
Agave validator: two successful fractional redemptions, one successful
permissionless escrow close, and five exact hostile refusals. The accepted
path burns all 40 shard atoms, pays all 16 collateral atoms, closes the
fractional record into the holder, reduces the escrow's outstanding count
from one to zero, and then closes the empty vault and escrow into the
permissionless cranker.

This is **historical-source-split local-validator evidence**. The host and
census were built from exact commit
`c444d4d459771505d53caf9bd5c04a180d9beb1f`. The executed Claims ELF is the
retained candidate from `551ffc1b99abdf01478e7e963a1ac20a99e6c0f6`, SHA-256
`b18309487b59dd39745e7937fe59e6e49eb6a983b1a0f4ea6c2f2ed056039ef2`.
Token-2022 SHA-256 is
`e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697`.
The two executable claim-check handlers and the three request/state codecs
they consume are byte-identical between the retained 551 source archive and
the c444 host source; `source-relation.sha256` records both sides. The Claims
crate root differs because c444 removed unrelated legacy retirement dispatch.
This run therefore does not claim final-source acceptance, devnet execution,
or mainnet execution.

## Finalized results

| label | slot | outcome | CU | exact poststate consequence |
|---|---:|---|---:|---|
| `dust-refusal` | 36 | `NoWholeClaim` (`0x5665`) | 14,520 | all protocol and Token-2022 accounts unchanged |
| `overdraw-refusal` | 68 | `Conservation` (`0x5663`) | 14,518 | all protocol and Token-2022 accounts unchanged |
| `substituted-payout-refusal` | 100 | `Authority` (`0x5661`) | 12,775 | all protocol and Token-2022 accounts unchanged |
| `substituted-holder-refusal` | 132 | `Authority` (`0x5661`) | 12,544 | all protocol and Token-2022 accounts unchanged |
| `partial-redemption` | 164 | accepted | 27,500 | supply/holder shards `40→20`; vault/record collateral `16→8`; holder collateral `0→8` |
| `premature-close-refusal` | 196 | `Vault` (`0x5625`) | 8,670 | record, escrow, vault, mints, and token accounts unchanged at the partial state |
| `settling-redemption` | 228 | accepted | 26,656 | supply/holder shards `20→0`; vault/record collateral `8→0`; holder collateral `8→16`; record closes and refunds 3,118,080 lamports; outstanding count `1→0` |
| `permissionless-close` | 260 | accepted | 12,226 | empty vault and escrow close; their live rent is credited to the fee-paying cranker |

Every row in `claim-check-native-evidence.json` contains the finalized signed
packet's signature, slot, fee, compute units, error, logs, resolved instruction
program ids and data, and exact poststate. The recorder byte-compared the
base64-decoded finalized packet with the bytes signed and submitted by the
host. It also required exactly one top-level Claims instruction with the
action's exact request bytes. The producer's independent verifier accepted all
eight ordered rows; the evidence digest is
`8c5cd60fef05c8d651406252d830b6339f6de6f724c8cf1fa052f247a40284d7`.

The exact signatures are authoritative in `claim-check-native-evidence.json`.
The accepted signatures are:

- partial redemption:
  `2NE3qthWCMrF7epZr8Ppy2smuE9vNwmnKeLPHftRTiDu9Fxe1JjhGuTyQxmuXvLhSi6ndZzfjELr4wGgdzMcKUir`
- settling redemption:
  `58oJZmU1a9yMatih4dVSZ613es2FGW12Pc1CtoWxZVjG5vfRMZnyoqaGmpfXB4KwCa5Dt5T5oQg2FQQp74YnNRVK`
- permissionless close:
  `5oGSmdK3YREzzACcMT3Dua5VXe56qtzDS2hb9MVxxMxQp5nfeKWefj7ZkKRCHDdezLPLsqATa3Jo2J8Rhc3TetLa`

## Census fold

The exact c444 inventory contains 162 routes and 456 refusal codes with zero
unclassified positions. `census observe` admitted eight
`finalized-instruction` observations with no problems:

- six rows for
  `claims/fractional_claim_check_v1::process_fractional_redemption`: two
  accepted and four refused;
- two rows for
  `claims/claim_check_redemption_v1::process_escrow_close#CloseEscrow`: one
  accepted and one refused.

The fold checks the program address and finalized instruction bytes before
admission. The close route additionally requires the exact 64-byte
`DCLTCCR1`, version 1, action 4 packet with canonical reserved fields and a
nonzero aggregate. Refused rows remain refusal evidence; they do not become
accepted route coverage. The selectorless `claims/process_instruction` entry
route was deliberately not credited from a same-program invocation.

## Commands and retained substrate

The exact c444 archive was at
`/tank/dregg-build/dclutch-claimcheck-c444d4d459771505d53caf9bd5c04a180d9beb1f-20260908`
with its own target at the parallel `-target` path. The native check preceded
the host build, both under `swarm-build` with `--locked --offline`:

```text
cargo check --locked --offline -p dclutch-fractional-exterior
cargo build --locked --offline -p dclutch-fractional-exterior
```

The fresh validator ledger and complete producer outputs remain at
`/home/hbox/dclutch-claim-check-native-c444d4d45-20260908`. The corpus copies
the compact durable outputs, not the validator ledger or staged genesis
accounts. `run.log`, `verify.log`, `inventory.log`, and `census-observe.log`
preserve the command results; `toolchain.txt` names the hbox and Agave
toolchains.

## Boundary still open

The starting claim-check record, escrow, vault, mints, and token accounts were
explicitly staged from the canonical post-compaction schema. This corpus does
not prove one continuous Open → terminal → compaction → redemption validator
journey, nor does it execute the compaction instruction. That continuity and
the retained terminal actions remain owned by the Claims validator campaign.
A final-source replay must rebuild the Claims ELF from one exact final commit,
run this same `run-claim-check` producer against it, and fold the resulting
native evidence through that commit's inventory with the unchanged bindings.
Only that replay can upgrade these accepted action witnesses from historical
source-split to final-source acceptance.
