# V2 foundation and three Direct cycles — 2026-09-09

The exact source revision `fd7e5fec68cb948a326b41dc69be76f27156da10`
completed a bounded owned-loopback local-validator run through Registry V2
publication, release activation, funded Direct founding/open, participant
admission, three nonzero Direct fills, fee settlement after every fill,
reconciliation, and a no-resend restart. This is local-validator evidence.

The sealed input is
`hbox:/tank/dregg-build/dclutch-strict-fd7e5fec6-v2-no-general-owner-20260909-run2`.
Its checked gate SHA-256 is
`3e9f1b4c9a269a257e88cf2a6b8da565a0868a2b267bd05ed991078e1b8b76ac`,
the campaign pack SHA-256 is
`c8f14757c784744be98ed4726f608ef5838e4c480882c18193db13b32e55140e`,
and the source-tree SHA-256 is
`05fb7efad13d45e91e65c462f3de3ed0b77dc0f03fc3f6773ddfca98ecc75b07`.
The sealed tree and its eight production ELFs were not modified.

The retained validator ledger is under
`hbox:/tank/dregg-build/dclutch-v2-validator-floor-fd7e5fec6-20260909-run1`.
The consolidated foundation result is `V2_FOUNDATION_RESULT.json`, SHA-256
`e7e9ae0c6a217a0673cfcf98edbc61d9ee5763747c5ccc9a981d0a84af30b643`.
Registry published 11 distinct records in 37 publication transactions,
including 15 ArtifactReleaseV2 Finalize transactions. The largest accepted
Finalize consumed 548,405 CU at slot 162, signature
`3wCkUSiaQ3uCQwoWzGN3PPgEaKXTq957wpY75xaEJdHzGjGCWvCjwcYRxyXb5MK3AnG9XT3GnGF5yPkgR851xXTS`.
The largest release-set activation consumed 41,518 CU at slot 1378, signature
`5E47xhST43dq6kG6Pdmitr22zGR51Vr6B86siSbheYSL3NjeuLnWzZynCVXjevDUJryjYxWuwFuk9DxLVYKW5car`.

Direct market `7yBj5vZWdt3r7WPUPGeqUq1vY2pdn2aBtPBGa5wx7gif` reached
Open. Its atomic Lock/Found/Realize/Claims/Open transaction consumed 875,254 CU
at slot 7935, signature
`5Bekug37KUaimH1svkRYnV9sVoiXBX2Gx5fhDZgPqTfLLPrXLyrfmjTUxorULTEAFsCmMzW1W4KUyPeGHKAEUPk3`.
Participant admission consumed 163,397 CU at slot 8360, signature
`2FYF6gtyZvzFjGosbpQHEJsAimjadgECK4KhyiRhKCRG15fQ9jkNBJPpvDSrSmyyXA4bpV6cNzW6CtNeikS2y9Q6`.
Its initial collateral transfer moved 50,250,000 atoms at slot 8405.

The validator was reopened from the same ledger without reset. All sampled
foundation transactions remained finalized and all eight programs remained
executable. The reopen check also caught a host-report defect: the participant
Position report predicted data SHA-256
`e928370a86b1f3370e282abe5ed597fec88194544402409eafbaffcb2ac24c51`,
while the authenticated bank held
`0ccca547e115703750961d916d0269fbdd7d7626023cd0d32cbf861d823dccc9`.
Commit `d96990bb` changed the host report to use authenticated finalized account
bytes. `RETAINED_BANK_REOPEN_PROOF.json` preserves the mismatch and transaction
survival, SHA-256
`4db884d3da26aff7f95d1c6ad6af0fdbe0e9cb343452d95d05964319533755c4`.

## Three funded fills

The simulator run is
`hbox:/tank/dregg-build/dclutch-v2-load-floor-fd7e5fec6-20260909-run3`.
The accepted host binary SHA-256 is
`da278d07fe383df1bc9484a89d7bd2b9ff839eac9fe67b97082497d47168cb7c`.

| Cycle | Fill atoms | Hot slot | Hot CU | Hot signature | Settled fee atoms |
| ---: | ---: | ---: | ---: | --- | ---: |
| 1 | 100,000,000 | 32161 | 1,038,225 | `624BFfesDf93u2EKcoYrdkjbN8yZEf3Kb3rh5GRjWLJC8xFrshgCTdryr5QUm5nxgFajyfDWYjjPjoNNK58Ujhfe` | 500,000 |
| 2 | 33,000,000 | 70652 | 1,020,726 | `7Fix1Rd9cSPSxGYsgPjLCs21G9KmhZffaJTWq15koqZygajfiMF4G9ZwSR8UiQ5nnnrCG6hhD4HXcqWu8PmFpgX` | 165,000 |
| 3 | 33,000,000 | 77826 | 1,020,726 | `5tUZhQ8Nm1t3wue5PLKci1pSMN23WuSm78UysHj9VbGE4tgd8MSi7z4eBsqdCYvyoJAPuNRHAJJk5uKvvnE1yhWy` | 165,000 |

Every settlement consumed 91,057 CU. Their slots were 44749, 76108, and
77910. Cycles 2 and 3 each used a separately finalized 16,582,500-atom Token
approval, and those approval signatures are retained in the outer cycle
journals. Cycle 1 predates the simulator settlement stage; its 500,000-atom
canonical settlement is retained separately and the finalized legacy cycle
journal was not rewritten.

The final census at slot 77910 holds 1,100,000,002 tracked collateral atoms,
equal to Mint supply. Hoard principal remains 500,000,001. Aggregate supply is
`[166666667, 166666667, 166666667, 166666667]`. The buyer holds 166,000,000
outcome-0 claims and the founder holds 666,667; the seller token holds
82,585,000 collateral atoms, the venue fee token 830,000, the buyer token zero,
the separately admitted stranger token 16,582,500, and the fixture source
2,500. L1 through L6 and L8 hold. L7 is inapplicable because this external
census was not given a declared lamport-fee total.

The first cycle-2 census correctly halted with a 16,582,500-atom tracking gap:
the separately funded stranger token was absent from the configured token set.
The failed census is preserved at
`failed-census-cycle2-missing-stranger-token/cycle-000002.json`, SHA-256
`5c3f28b82607b18ef70675a1f7d2c431b64760b98d479c887088fc243e7fa53e`.
After adding the exact token address authenticated by its admission evidence,
the same boundary passed. `CYCLE2_CENSUS_CONFIG_CORRECTION.json` records the
change, SHA-256
`48d83766bbf4a0711856128f73dee2341a05afe2fe2fb5982029dc16e9c233c5`.

The restart reran the same three-cycle execute command. All 47 durable cycle,
session, transaction, settlement, and census artifacts remained byte-identical.
The fee-payer finalized signature set remained 72 before and after, with no new
signature. `THREE_CYCLE_RESUME_PROOF.json` has SHA-256
`d37cfa618e813045627e24d8f72d3b91638315271b0ce6851b7f4dd9a490d1a1`.

The consolidated machine-readable result is `THREE_CYCLE_V2_RESULT.json`,
SHA-256
`e0d9e0ab161fe0fffe476901ccb3123ed817376ba679bae364657401aa20601d`.
The validator and ledger remain live for the next retained-bank continuation;
no public RPC, wallet dotfile, devnet deployment, or other retained validator
was used or changed.
