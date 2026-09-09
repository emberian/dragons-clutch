# Devnet redeploy inputs — 2026-09-07

Owner: CODEX-INTEGRATION. Bounded read-only readiness check; no deployment or
funding transaction was submitted.

The cohort-17 job's `cohort17.env` records the required deployer key path.
The file exists, and `solana-keygen pubkey` reproduced the expected public key:
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`. No key material is included here.
The missing local CLI default path was not the project's recorded key path.

The RPC batch was issued from live source
`418f4195c3e84a3a54c6620618479cf59eff8e0d` in
`/Users/ember/dev/dclutch`, against `https://api.devnet.solana.com`.
It contained one genesis read, one finalized balance read and nine rent queries.
Genesis was `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`.
At finalized balance context slot `494893178`, the deployer held
`26,572,399,090` lamports (`26.572399090` devnet SOL).

The queried ProgramData lengths were each ELF length plus the Loader-v3
45-byte header. ELF sizes came from the sealed eight-program a7f9f226 candidate
on hbox; this is an intermediate runtime, not the final redeploy source.

| Role | ELF bytes | RPC ProgramData rent (lamports) |
| --- | ---: | ---: |
| Accelerator | 831,408 | 4,224,431,480 |
| Claims | 1,458,128 | 7,408,169,080 |
| Core | 1,205,712 | 6,125,895,800 |
| Custody | 468,768 | 2,382,220,280 |
| Registry | 237,696 | 1,208,374,520 |
| Rent | 143,152 | 728,091,000 |
| Resolution | 932,984 | 4,740,437,560 |
| Trading | 2,425,880 | 12,324,349,240 |

The 36-byte Program-account quote was `833,120` lamports each. The eight
ProgramData accounts plus eight Program accounts require `39,148,633,920`
lamports (`39.148633920` SOL) at these quoted sizes. The rent-only gap against
the observed balance is `12,576,234,830` lamports (`12.576234830` SOL).

Transaction fees, temporary upload buffers and simulator funding are additional.
The final committed ELF sizes and RPC rent quotes must determine the actual
deployment spend. The old cohort remains in place under current authorization.

## 2026-09-08 addendum — bounded balance refresh

At `2026-09-08T15:43:53Z`, two public RPC reads from
`/Users/ember/dev/dclutch` at source
`f749f3e2bdee79caa04da37b9e1b1763245a1b62` rechecked genesis and finalized
balance. Genesis still matched the manifest's devnet identity. At finalized
slot `495175150`, the same deployer still held `26,572,399,090` lamports.
No funding or deployment transaction was submitted. The old ELF rent table
above remains a historical quote; final committed ELF lengths require new
quotes before deployment. The response is retained at
`/private/tmp/dclutch-devnet-balance-refresh-20260908.json`.

## 2026-09-09 addendum — V2 candidate rent quote

A bounded batch of eleven reads (genesis, finalized deployer balance, eight
ProgramData rents and one Program rent) used the same public endpoint at
`2026-09-09T05:01:18.335923+00:00`, from live source
`67acd0ff9ca4294c889422a4526e8990e48c739f`. The artifact source was the sealed
`fd7e5fec68cb948a326b41dc69be76f27156da10` candidate; its production ELF
lengths were Accelerator 833,024, Claims 1,460,240, Core 1,174,368, Custody
470,400, Registry 251,024, Rent 143,496, Resolution 998,664 and Trading
2,737,576 bytes. Each ProgramData quote included the 45-byte Loader header.

Genesis matched devnet. At finalized slot `495463740`, the same deployer
held `26,572,399,090` lamports. The eight ProgramData accounts plus eight
Program accounts quoted `41,003,159,040` lamports: a rent-only shortfall of
`14,430,759,950` lamports (14.430759950 devnet SOL). Fees, upload working
capital and simulator funding remain additional. Final completion-source
artifacts still require their own quotes before deployment; this observation
submits no funding or deployment transaction.

The request/response record is
`/private/tmp/dclutch-devnet-rent-fd7-20260909.json`, SHA-256
`b2087c9a79bdb5d96a654c848615cbde61be99bed3315152b024568c095641e7`.
